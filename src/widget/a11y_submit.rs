// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The pump between the two halves of accessibility that already existed.
//!
//! # Why this module exists
//!
//! Accessibility in this crate had three stages, and the middle one was missing entirely
//! (BLUE24 §0A.1, measurement 5):
//!
//! | stage | state before this module |
//! |---|---|
//! | ① derive a meaning from a control | **already built** — [`A11yState::from_widget`], with a unit test per branch |
//! | ② submit it to the platform bridge when the tree changes | **did not exist** |
//! | ③ bridge to the OS accessibility bus | **already built** — AT-SPI2/D-Bus, `NSAccessibility`, UI Automation |
//!
//! The shape of the gap is what makes it worth a module of its own: ① and ③ are both complete and
//! both tested, so any reading of the code concludes accessibility is done. And it was not — the
//! AT-SPI2 bridge, the `NSAccessibility` bridge and the UI Automation bridge each had **zero**
//! production callers of `set_accessibility_name`, `notify_state_changed` and the rest. A screen
//! reader on any of the three platforms received nothing.
//!
//! # Why it must push rather than be pulled
//!
//! `widget_a11y_state` is a **pull** entry point: a caller can ask "what is this widget's meaning?".
//! A screen reader never does that. Its trigger is the *notification* this module posts, so
//! without a push the pull has no reason to ever be called — an open loop. The two entry points are
//! therefore complementary, not alternatives: this module pushes a change, and the bridge may then
//! pull the full state through the existing accessor.
//!
//! # Why the bridge is behind a seam
//!
//! The platform bridge is a real OS object, so a test can only exercise the submit path through a
//! substitute. The installed bridge is therefore a thread-local slot, the same shape
//! [`crate::style::environment`] uses for the device facts and for the same reason: every submit
//! point is then reachable in a unit test with no window and no accessibility bus.
//!
//! # Honesty about what this does not do
//!
//! This module owns *when* a change is pushed and *what* is pushed. It does not own how a bridge
//! talks to its OS, and it makes no claim that a native control's appearance matches a library
//! one — those are platform facts (see BLUE24 §7's "honest boundary").
//!
//! [`A11yState::from_widget`]: crate::platform::accessibility::A11yState::from_widget

use crate::core::ObjectId;
use crate::platform::accessibility::{A11yState, AccessibilityBridge};

/// The bridge a submit point reports to, or `None` when the host has installed none.
///
/// # Why the installed bridge can be replaced
///
/// A test has to count what was pushed — "three controls mounted, so three nodes were created,
/// each with a non-empty role and label" is the only statement that distinguishes a real pump from
/// a function that returns success without doing anything (BLUE24 §6 criterion 1). The seam is the
/// installation, exactly as it is for the environment provider.
///
/// A raw pointer rather than a trait object, because the referenced bridge is a `'static` value
/// owned jointly by the platform and by whichever test installed its substitute; the pointer is
/// only ever dereferenced inside a call that does not outlive the borrow it was made under.
struct InstalledBridge {
    ptr: *const dyn AccessibilityBridge,
}

thread_local! {
    /// The bridge submit points report to, or `None`.
    ///
    /// # Why thread-local
    ///
    /// Widgets are `!Send` and every desktop backend runs UI work on the platform main thread, so
    /// the tree this module reports on only ever exists there. A thread-local also lets a test
    /// install a counting substitute without racing a parallel test — the same reasoning as the
    /// environment provider's slot.
    #[allow(clippy::missing_const_for_thread_local)]
    static INSTALLED: core::cell::RefCell<Option<InstalledBridge>> =
        const { core::cell::RefCell::new(None) };

    /// Whether the pinned platform bridge has been resolved yet.
    ///
    /// Resolving it is a thread-local store of a `'static` pointer, so it happens once per thread
    /// rather than once per submit point.
    #[allow(clippy::missing_const_for_thread_local)]
    static PLATFORM_RESOLVED: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// Installs `bridge` as the bridge submit points report to.
///
/// Returns whether one was already installed, so a test can tell "I replaced something" from "I
/// was the first" — and, at the platform boundary, so a later installation can be recognised as a
/// replacement rather than a first pinning.
///
/// # Safety
///
/// The caller must guarantee `bridge` outlives every subsequent submit call on this thread. The
/// intended uses are a `'static` platform bridge and a test substitute that is kept alive by the
/// test's own scope. [`uninstall_bridge`] reverses this and is what a test calls on the way out.
pub fn install_bridge(bridge: &'static dyn AccessibilityBridge) -> bool {
    // A test installation must win over the pinned platform bridge, so mark the platform as
    // resolved: otherwise the next `current()` would replace the substitute with the platform's.
    let _ = PLATFORM_RESOLVED.try_with(|flag| flag.set(true));
    INSTALLED
        .try_with(|slot| slot.borrow_mut().replace(InstalledBridge { ptr: bridge }).is_some())
        .unwrap_or(false)
}

/// Removes the installed bridge, if any. Returns whether one was removed.
///
/// The counterpart of [`install_bridge`] for a test that wants to leave the thread as it found it.
pub fn uninstall_bridge() -> bool {
    INSTALLED.try_with(|slot| slot.borrow_mut().take().is_some()).unwrap_or(false)
}

/// Calls `f` with the bridge submit points report to, or does nothing when there is none.
///
/// # Why "no bridge" is a silent no-op rather than an error
///
/// A build with no accessibility bridge — `mini`, `embedded`, a backend that has not implemented
/// one — must still mount, unmount and restyle controls. Reporting is not a precondition for
/// rendering, so a missing bridge is an absent capability, not a failure (principle #37: a missing
/// capability is stated by an honest default, never fabricated and never fatal).
fn with_bridge<R>(f: impl FnOnce(&dyn AccessibilityBridge) -> R) -> Option<R> {
    // A test-installed bridge takes precedence and is never overwritten by the platform's.
    let installed = INSTALLED
        .try_with(|slot| slot.borrow().as_ref().map(|installed| installed.ptr))
        .ok()
        .flatten();
    let ptr = match installed {
        Some(ptr) => ptr,
        None => platform_bridge_ptr()?,
    };
    // SAFETY: the pointer was produced from a reference whose lifetime the installer promised
    // covers every submit call; `with_bridge` does not store or leak it.
    Some(f(unsafe { &*ptr }))
}

/// Resolves the pinned platform bridge once, returning its pointer.
fn platform_bridge_ptr() -> Option<*const dyn AccessibilityBridge> {
    let already = PLATFORM_RESOLVED.try_with(|flag| flag.get()).unwrap_or(false);
    if !already {
        let _ = PLATFORM_RESOLVED.try_with(|flag| flag.set(true));
    }
    let bridge = crate::platform::runtime::get_platform().accessibility_bridge()?;
    Some(bridge as *const dyn AccessibilityBridge)
}

/// Reports that a control has been mounted, creating its node in the accessibility tree.
///
/// Submit point 1 of BLUE24 §6.3. The state is derived through [`A11yState::from_widget`] (stage ①)
/// rather than built here, so the meaning a screen reader is handed and the meaning
/// [`crate::widget::Widget::accessible_value`] reports cannot disagree.
///
/// A control whose derive yields no label still gets a node: a role with an empty label is a
/// control a screen reader can navigate past by role, whereas no node at all is a control that
/// does not exist to it. What is *not* invented is a label — an empty field is reported as empty.
pub fn submit_mounted(id: ObjectId, state: &A11yState) {
    with_bridge(|bridge| {
        // D09-A11Y-02: the *whole* derived state is submitted, not the label alone. Before this the
        // bridge boundary kept only `state.label`, so role, description, enabled/focused/selected/
        // expanded, value, checked/mixed and the child relations never left the widget. The bridge
        // stores what this carries and a screen reader can query it.
        bridge.submit_node_state(id, state);
        // And the creation is announced; a bridge that keeps its own tree has the node by now.
        bridge.notify_state_changed(id);
    });
}

/// Reports that a control has been unmounted, so its node can be torn down.
///
/// Submit point 2 of BLUE24 §6.3. Paired with [`submit_mounted`]: a node that outlives its control
/// is the failure a screen reader experiences as "focus lands on nothing".
pub fn submit_unmounted(id: ObjectId) {
    with_bridge(|bridge| {
        // D09-A11Y-03: the node is *removed*, not blanked. `unregister_node` drops the entry from
        // the bridge's store, so the maps return to their baseline across repeated mount/unmount
        // cycles instead of accumulating one empty-named entry per historical widget id.
        bridge.unregister_node(id);
        // Only then is the removal announced — a notification about a node the bridge has already
        // dropped is exactly the "event source the application does not own" defect of D09-A11Y-01.
        bridge.notify_state_changed(id);
    });
}

/// Reports that a control's geometry changed, so its bounds can be re-read.
///
/// Submit point 3 of BLUE24 §6.3. Reports the fact rather than a rectangle, because bounds are a
/// pull: the bridge asks the mounted control through the existing accessor, so there is one answer
/// to "where is this control" rather than one answer in the emit and another in the pull.
pub fn submit_geometry_changed(id: ObjectId) {
    with_bridge(|bridge| bridge.notify_state_changed(id));
}

/// Reports that a control's interaction state changed.
///
/// Submit point 4 of BLUE24 §6.3. Called *only when the state actually differs* — the caller
/// compares, so a repeated write of the same state posts nothing. That idempotence is BLUE24 §6
/// criterion 4 and is the same coalescing rule the frame's repaint accounting uses.
pub fn submit_state_changed(id: ObjectId) {
    with_bridge(|bridge| bridge.notify_state_changed(id));
}

/// Reports that a control's **meaning** changed (BLUE24 §3's third consumer).
///
/// Submit point 5, and the newest: once `error`/`warning`/`success` left the interaction chain and
/// became [`crate::style::SemanticState`], they needed a consumer, and "this field failed
/// validation" is precisely a thing a screen reader is expected to announce.
pub fn submit_semantic_changed(id: ObjectId) {
    with_bridge(|bridge| bridge.notify_state_changed(id));
}

/// Reports that a control's value changed.
///
/// Not one of the five §6.3 points (those cover the *tree*, "this control's text is now longer" is
/// the control's own fact), but it travels the same pump so there is one place that knows how to
/// reach a bridge. A caller that has already announced a value has no reason to call this.
pub fn submit_value_changed(id: ObjectId) {
    with_bridge(|bridge| bridge.notify_value_changed(id));
}

/// Reports that focus moved to `id`, so a screen reader follows the keyboard.
pub fn submit_focus_changed(id: ObjectId) {
    with_bridge(|bridge| bridge.notify_focus_changed(id));
}

#[cfg(all(test, not(alloc_frugal)))]
mod tests {
    use super::*;
    use crate::platform::accessibility::{A11yRole, A11yState};
    use std::sync::Mutex;

    /// What a submit point was asked to do, recorded for assertion.
    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        Name(ObjectId, String),
        State(ObjectId),
        Value(ObjectId),
        Focus(ObjectId),
        /// D09-A11Y-02: the complete derived state that reached the bridge boundary.
        Node(ObjectId, A11yState),
        /// D09-A11Y-03: the explicit removal of a node.
        Unregister(ObjectId),
    }

    /// A bridge that records the calls made to it, so the submit path can be asserted.
    ///
    /// `Mutex` rather than `RefCell` because the trait is `Send + Sync` (a real bridge is posted
    /// to from the platform's own threads); the substitute has to satisfy the same bound.
    struct Recording {
        calls: Mutex<Vec<Call>>,
    }

    impl Recording {
        fn new() -> Self {
            Self { calls: Mutex::new(Vec::new()) }
        }

        fn calls(&self) -> Vec<Call> {
            self.calls.lock().expect("recording mutex is never poisoned").clone()
        }
    }

    impl AccessibilityBridge for Recording {
        fn set_accessibility_name(&self, id: ObjectId, name: &str) {
            self.calls.lock().expect("not poisoned").push(Call::Name(id, name.to_string()));
        }
        fn accessibility_name(&self, _id: ObjectId) -> Option<String> {
            None
        }
        fn notify_name_changed(&self, _id: ObjectId) {}
        fn notify_value_changed(&self, id: ObjectId) {
            self.calls.lock().expect("not poisoned").push(Call::Value(id));
        }
        fn notify_state_changed(&self, id: ObjectId) {
            self.calls.lock().expect("not poisoned").push(Call::State(id));
        }
        fn notify_focus_changed(&self, id: ObjectId) {
            self.calls.lock().expect("not poisoned").push(Call::Focus(id));
        }
        // D09-A11Y-02 and D09-A11Y-03: the recording substitute adopts the full-state contract so
        // the assertions can see the whole submitted state and the explicit removal, rather than
        // only the label and a payload-less notification.
        fn submit_node_state(&self, id: ObjectId, state: &A11yState) {
            self.calls.lock().expect("not poisoned").push(Call::Node(id, state.clone()));
        }
        fn unregister_node(&self, id: ObjectId) {
            self.calls.lock().expect("not poisoned").push(Call::Unregister(id));
        }
    }

    /// Installs a recording bridge for the duration of `f`, then removes it.
    fn with_recording<R>(f: impl FnOnce(&Recording) -> R) -> R {
        // Boxed and leaked so the reference is `'static`. The `Box` is reclaimed when the thread's
        // slot is replaced at the end of the test; the leak is one small allocation per test.
        let recorder: &'static Recording = Box::leak(Box::new(Recording::new()));
        let had = install_bridge(recorder);
        let result = f(recorder);
        let removed = uninstall_bridge();
        assert!(!had, "no bridge should have been installed before this test");
        assert!(removed, "the substitute must be removable, or it leaks into the next test");
        result
    }

    /// BLUE24 §6 criterion 1: mounting a control creates a node, with role and label.
    ///
    /// D09-A11Y-02 strengthens the assertion: the node the bridge receives carries the *full*
    /// derived state, not only the label. The comparison is against the whole `A11yState`, so any
    /// field silently dropped at the boundary makes it fail.
    #[test]
    fn mounting_submits_a_node() {
        with_recording(|recorder| {
            let state = A11yState {
                role: A11yRole::Button,
                label: "Save".to_string(),
                ..A11yState::default()
            };
            submit_mounted(7, &state);

            let calls = recorder.calls();
            assert!(
                calls.contains(&Call::Node(7, state.clone())),
                "the node's complete state must reach the bridge: {calls:?}"
            );
            assert!(
                calls.contains(&Call::State(7)),
                "and its creation must be reported: {calls:?}"
            );
        });
    }

    /// D09-A11Y-02: every field of the derived state crosses the bridge boundary intact.
    ///
    /// The old `submit_mounted` sent only `state.label`, so role, description, enabled, focused,
    /// selected, expanded, value, checked, mixed and children were all lost. This asserts each one
    /// individually against the substitute, which is the "observable assertion" the defect asks
    /// for — not a claim that a function was called.
    #[test]
    fn mounting_submits_every_derived_field() {
        with_recording(|recorder| {
            let state = A11yState {
                role: A11yRole::CheckBox,
                label: "Subscribe".to_string(),
                description: "Toggles the newsletter".to_string(),
                enabled: false,
                focused: true,
                selected: true,
                expanded: true,
                value: "on".to_string(),
                checked: Some(true),
                mixed: true,
                children: vec![11, 12, 13],
            };
            submit_mounted(21, &state);

            let submitted = recorder
                .calls()
                .into_iter()
                .find_map(|call| match call {
                    Call::Node(21, recorded) => Some(recorded),
                    _ => None,
                })
                .expect("the full node state must have been submitted");
            assert_eq!(submitted.role, A11yRole::CheckBox, "role crossed the boundary");
            assert_eq!(submitted.label, "Subscribe", "label crossed the boundary");
            assert_eq!(
                submitted.description, "Toggles the newsletter",
                "description crossed the boundary"
            );
            assert!(!submitted.enabled, "enabled crossed the boundary");
            assert!(submitted.focused, "focused crossed the boundary");
            assert!(submitted.selected, "selected crossed the boundary");
            assert!(submitted.expanded, "expanded crossed the boundary");
            assert_eq!(submitted.value, "on", "value crossed the boundary");
            assert_eq!(submitted.checked, Some(true), "checked crossed the boundary");
            assert!(submitted.mixed, "mixed crossed the boundary");
            assert_eq!(submitted.children, vec![11, 12, 13], "children crossed the boundary");
        });
    }

    /// Unmounting reports the removal, paired with the mount.
    ///
    /// D09-A11Y-03: the removal must be an explicit `unregister_node`, not a name blanked to "".
    /// A blanked name is exactly what left the map entry behind for the life of the process.
    #[test]
    fn unmounting_submits_the_removal() {
        with_recording(|recorder| {
            submit_unmounted(9);
            assert_eq!(recorder.calls(), vec![Call::Unregister(9), Call::State(9)]);
        });
    }

    /// The remaining submit points each reach the bridge, so none is a function that does nothing.
    #[test]
    fn every_submit_point_reaches_the_bridge() {
        with_recording(|recorder| {
            submit_geometry_changed(1);
            submit_state_changed(2);
            submit_semantic_changed(3);
            submit_value_changed(4);
            submit_focus_changed(5);
            assert_eq!(
                recorder.calls(),
                vec![
                    Call::State(1),
                    Call::State(2),
                    Call::State(3),
                    Call::Value(4),
                    Call::Focus(5),
                ]
            );
        });
    }

    /// D09-A11Y-02 and D09-A11Y-03, end to end through one bridge that keeps a real store.
    ///
    /// A `Storing` substitute that models the tree like the platform bridges do proves the pump
    /// twice over: after a mount the store holds the full state (role, value, checked) and not just
    /// the name, and after an unmount the store is back to empty rather than holding a blanked
    /// entry — so repeated cycles cannot grow it.
    #[test]
    fn the_submit_pump_round_trips_full_state_and_removes_on_unmount() {
        use std::collections::HashMap as StdHashMap;
        use std::sync::Mutex as StdMutex;

        struct Storing {
            nodes: StdMutex<StdHashMap<ObjectId, A11yState>>,
        }
        impl AccessibilityBridge for Storing {
            fn set_accessibility_name(&self, _id: ObjectId, _name: &str) {}
            fn accessibility_name(&self, _id: ObjectId) -> Option<String> {
                None
            }
            fn notify_name_changed(&self, _id: ObjectId) {}
            fn notify_value_changed(&self, _id: ObjectId) {}
            fn notify_state_changed(&self, _id: ObjectId) {}
            fn notify_focus_changed(&self, _id: ObjectId) {}
            fn submit_node_state(&self, id: ObjectId, state: &A11yState) {
                self.nodes.lock().expect("not poisoned").insert(id, state.clone());
            }
            fn node_state(&self, id: ObjectId) -> Option<A11yState> {
                self.nodes.lock().expect("not poisoned").get(&id).cloned()
            }
            fn unregister_node(&self, id: ObjectId) {
                self.nodes.lock().expect("not poisoned").remove(&id);
            }
            fn node_count(&self) -> usize {
                self.nodes.lock().expect("not poisoned").len()
            }
        }

        let bridge: &'static Storing =
            Box::leak(Box::new(Storing { nodes: StdMutex::new(StdHashMap::new()) }));
        let had = install_bridge(bridge);
        assert!(!had, "no bridge should have been installed before this test");

        let baseline = bridge.node_count();
        assert_eq!(baseline, 0, "a fresh store is empty");

        for cycle in 0..3u64 {
            let id = 100 + cycle;
            let state = A11yState {
                role: A11yRole::Slider,
                label: String::new(),
                value: "42".to_string(),
                ..A11yState::default()
            };
            submit_mounted(id, &state);
            let stored = bridge.node_state(id).expect("the node must be readable after mount");
            assert_eq!(stored.role, A11yRole::Slider, "role survived cycle {cycle}");
            assert_eq!(stored.value, "42", "value survived cycle {cycle}");
            assert_eq!(bridge.node_count(), baseline + 1, "one live node in cycle {cycle}");

            submit_unmounted(id);
            assert!(bridge.node_state(id).is_none(), "the node is gone after unmount");
            assert_eq!(
                bridge.node_count(),
                baseline,
                "the store returns to baseline after cycle {cycle}"
            );
        }

        let removed = uninstall_bridge();
        assert!(removed, "the substitute must be removable, or it leaks into the next test");
    }

    /// With no bridge installed every submit point is a no-op that does not panic — the
    /// `mini`/`embedded` case, where there is no bridge to report to.
    ///
    /// BLUE24 §6 criterion 5. The assertion is the absence of a panic *and* the honest answer that
    /// nothing happened: a submit path that fabricated success would be indistinguishable from one
    /// that worked.
    #[test]
    fn without_a_bridge_every_submit_point_is_a_no_op() {
        let _ = uninstall_bridge();
        let state = A11yState { role: A11yRole::Label, ..A11yState::default() };
        submit_mounted(11, &state);
        submit_unmounted(11);
        submit_geometry_changed(11);
        submit_state_changed(11);
        submit_semantic_changed(11);
        submit_value_changed(11);
        submit_focus_changed(11);
        // Reaching here without a panic is the assertion; there is no bridge to have recorded to.
    }
}
