// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Event propagation: capture, target and bubble phases with an explicit consumed flag.
//!
//! # The gap this closes
//!
//! A flat dispatch delivers an event to exactly one control. Every real interaction tree needs
//! more than that:
//!
//! - a **key** that a focused control ignores must reach a shortcut handler on an ancestor
//!   (a form that handles `Escape` at the dialog level);
//! - a **click** on a row inside a list must let the list see it after the row;
//! - a **capture** handler must be able to intercept before the target (a modal that swallows
//!   pointer input, a scroll area that pre-empts a drag).
//!
//! All three are one mechanism: deliver down the ancestor chain, to the target, then back up the
//! chain, stopping wherever a handler says it consumed the event.
//!
//! # Why the consumed flag is thread-local
//!
//! The handler signature the library already publishes is `Fn(ObjectId, &Event)` — it has no
//! return value. Changing that signature would touch every control, so the "I handled this"
//! answer travels through a per-thread flag instead: a handler calls [`consume_event`], and the
//! router reads [`take_consumed`] between deliveries and clears it. The flag is per-thread
//! because events are dispatched on the UI thread; a handler on a worker thread has no router to
//! talk to and must not affect one.
//!
//! A handler that consumes an event is asserting that its ancestors should not see it. A
//! handler that does nothing lets it continue, which is the correct default: most handlers
//! ignore most events.

use crate::compat::Vec;
use crate::core::ObjectId;
use crate::event::Event;

/// Which stage of the route is being delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchPhase {
    /// Top-down, before the target: the interception stage.
    Capture,
    /// The target itself.
    Target,
    /// Bottom-up, after the target: the default handling stage.
    Bubble,
}

/// Whether an event is still travelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Propagation {
    /// No handler claimed the event; the router continues.
    #[default]
    Continue,
    /// A handler claimed the event; the router stops.
    Consumed,
}

thread_local! {
    /// Set by a handler through [`consume_event`], read and cleared by the router.
    static CONSUMED: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// Marks the current event as handled, so the router stops travelling up or down the chain.
///
/// Returns the previous value: `false` means this call is what consumed the event, `true` means
/// a handler earlier in the same delivery had already done so.
pub fn consume_event() -> bool {
    CONSUMED.try_with(|cell| cell.replace(true)).unwrap_or(false)
}

/// Reads the consumed flag without clearing it.
pub fn is_consumed() -> bool {
    CONSUMED.try_with(|cell| cell.get()).unwrap_or(false)
}

/// Reads and clears the consumed flag, returning whether the event was consumed.
///
/// The router calls this between deliveries, so one handler's consumption cannot leak into the
/// next event. A handler that wants to ask "was this already consumed?" uses [`is_consumed`].
pub fn take_consumed() -> bool {
    CONSUMED.try_with(|cell| cell.replace(false)).unwrap_or(false)
}

/// Clears the consumed flag.
///
/// Called at the start of a route so a stale `true` from an event whose handler panicked cannot
/// stop the next one before any handler has run.
pub fn reset_consumed() {
    let _ = CONSUMED.try_with(|cell| cell.set(false));
}

/// Delivers `event` through capture, target and bubble, stopping wherever a handler consumes it.
///
/// `ancestors` is ordered **root first** (the outermost ancestor at index 0), so the capture
/// pass walks it forwards and the bubble pass backwards. The target is not required to appear in
/// `ancestors`; a caller that has the parent chain is expected to pass the chain *above* the
/// target.
///
/// `handler` is called once per phase per node and is expected to call [`consume_event`] when it
/// does not want the event to travel further. The return value describes the final state:
/// [`Propagation::Consumed`] when a handler claimed the event, [`Propagation::Continue`] when the
/// whole route completed untouched.
///
/// # Why capture is opt-in per handler
///
/// The same handler is called for all three phases, with the phase as an argument. A handler
/// that only cares about the target simply ignores the other two phases; a handler that wants
/// to intercept checks for [`DispatchPhase::Capture`]. Splitting the trait into three would be
/// three methods to implement on every control for a capability most of them do not use.
pub fn route_event(
    target: ObjectId,
    ancestors: &[ObjectId],
    event: &Event,
    handler: &mut impl FnMut(ObjectId, &Event, DispatchPhase),
) -> Propagation {
    reset_consumed();

    // Capture: outermost first, so a modal at the root can intercept before a child sees the
    // event at all.
    for &ancestor in ancestors.iter() {
        handler(ancestor, event, DispatchPhase::Capture);
        if take_consumed() {
            return Propagation::Consumed;
        }
    }

    // Target.
    handler(target, event, DispatchPhase::Target);
    if take_consumed() {
        return Propagation::Consumed;
    }

    // Bubble: nearest ancestor first, which is the order a control's own default handler
    // expects when it delegates to its container.
    for &ancestor in ancestors.iter().rev() {
        handler(ancestor, event, DispatchPhase::Bubble);
        if take_consumed() {
            return Propagation::Consumed;
        }
    }

    Propagation::Continue
}

/// Builds the root-first ancestor chain for `target` by walking `parent_of` upward.
///
/// The walk is bounded by the number of steps it can take (a corrupt parent map is a cycle, and
/// a cycle must not hang the loop), so a caller gets a finite answer even from bad data.
pub fn ancestor_chain(
    target: ObjectId,
    parent_of: &dyn Fn(ObjectId) -> Option<ObjectId>,
) -> Vec<ObjectId> {
    let mut chain = Vec::new();
    fill_ancestor_chain(target, parent_of, &mut chain);
    chain
}

fn fill_ancestor_chain(
    target: ObjectId,
    parent_of: &dyn Fn(ObjectId) -> Option<ObjectId>,
    chain: &mut Vec<ObjectId>,
) {
    chain.clear();
    let mut current = parent_of(target);
    // The bound is generous for a UI tree and finite for a cyclic one.
    for _ in 0..1024 {
        let Some(parent) = current else {
            break;
        };
        chain.push(parent);
        current = parent_of(parent);
    }
    chain.reverse();
}

thread_local! {
    static ANCESTOR_BUFFER: core::cell::RefCell<Vec<ObjectId>> =
        const { core::cell::RefCell::new(Vec::new()) };
}

struct AncestorBuffer(Vec<ObjectId>);

impl Drop for AncestorBuffer {
    fn drop(&mut self) {
        self.0.clear();
        let _ = ANCESTOR_BUFFER.try_with(|slot| {
            // Drops run LIFO, so the outermost loan runs last and its storage is what the next
            // top-level dispatch reuses. Returning it unconditionally (rather than keeping
            // whichever buffer happens to be larger) is what makes a caller's buffer address
            // stable across a nested dispatch; a nested buffer is simply dropped instead of
            // displacing the outer one.
            *slot.borrow_mut() = core::mem::take(&mut self.0);
        });
    }
}

pub(crate) fn with_ancestor_chain<R>(
    target: ObjectId,
    parent_of: &dyn Fn(ObjectId) -> Option<ObjectId>,
    f: impl FnOnce(&[ObjectId]) -> R,
) -> R {
    // Lend the buffer out without holding the TLS borrow across callbacks. Nested dispatch gets
    // its own buffer, and Drop returns storage even when a parent resolver or handler panics.
    let mut buffer =
        AncestorBuffer(ANCESTOR_BUFFER.with(|slot| core::mem::take(&mut *slot.borrow_mut())));
    fill_ancestor_chain(target, parent_of, &mut buffer.0);
    f(&buffer.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::vec;

    /// A click that nobody consumes visits capture, target and bubble in order.
    #[test]
    fn an_unconsumed_event_travels_the_whole_route() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&seen);
        let propagation = route_event(3, &[1, 2], &Event::Paint, &mut |node, _, phase| {
            log.borrow_mut().push((node, phase));
        });
        assert_eq!(propagation, Propagation::Continue);
        let seen = seen.borrow();
        assert_eq!(
            *seen,
            vec![
                (1, DispatchPhase::Capture),
                (2, DispatchPhase::Capture),
                (3, DispatchPhase::Target),
                (2, DispatchPhase::Bubble),
                (1, DispatchPhase::Bubble),
            ]
        );
    }

    /// A target that consumes stops the bubble pass at once.
    #[test]
    fn a_consuming_target_stops_the_bubble_pass() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&seen);
        let propagation = route_event(3, &[1, 2], &Event::Paint, &mut |node, _, phase| {
            log.borrow_mut().push((node, phase));
            if phase == DispatchPhase::Target {
                consume_event();
            }
        });
        assert_eq!(propagation, Propagation::Consumed);
        assert!(
            !seen.borrow().iter().any(|(_, phase)| *phase == DispatchPhase::Bubble),
            "nothing above the target may see a consumed event"
        );
    }

    /// A capture handler can intercept before the target runs.
    #[test]
    fn a_capture_handler_can_pre_empt_the_target() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&seen);
        let propagation = route_event(3, &[1, 2], &Event::Paint, &mut |node, _, phase| {
            log.borrow_mut().push((node, phase));
            if node == 1 && phase == DispatchPhase::Capture {
                consume_event();
            }
        });
        assert_eq!(propagation, Propagation::Consumed);
        assert_eq!(*seen.borrow(), vec![(1, DispatchPhase::Capture)]);
    }

    /// A stale flag from an earlier route cannot stop the next one.
    #[test]
    fn a_stale_consumed_flag_does_not_leak_between_routes() {
        consume_event();
        let propagation = route_event(1, &[], &Event::Paint, &mut |_, _, _| {});
        assert_eq!(propagation, Propagation::Continue);
    }

    #[test]
    fn cached_ancestor_storage_survives_nested_dispatch_and_unwind() {
        let parent = |id| if id > 1 { Some(id - 1) } else { None };
        let address = with_ancestor_chain(4, &parent, |outer| {
            assert_eq!(outer, &[1, 2, 3]);
            with_ancestor_chain(3, &parent, |inner| assert_eq!(inner, &[1, 2]));
            assert_eq!(outer, &[1, 2, 3]);
            outer.as_ptr() as usize
        });
        with_ancestor_chain(4, &parent, |chain| assert_eq!(chain.as_ptr() as usize, address));
        let outcome = std::panic::catch_unwind(|| {
            with_ancestor_chain(4, &parent, |_| panic!("handler failure"));
        });
        assert!(outcome.is_err());
        with_ancestor_chain(4, &parent, |chain| {
            assert_eq!(chain, &[1, 2, 3]);
            assert_eq!(chain.as_ptr() as usize, address);
        });
        with_ancestor_chain(5, &|_| Some(5), |chain| assert_eq!(chain.len(), 1024));
    }

    /// The ancestor chain is root-first and bounded.
    #[test]
    fn the_ancestor_chain_is_root_first() {
        let parent = |id: ObjectId| match id {
            3 => Some(2),
            2 => Some(1),
            _ => None,
        };
        assert_eq!(ancestor_chain(3, &parent), vec![1, 2]);
        let cyclic = |_id: ObjectId| Some(5);
        assert_eq!(ancestor_chain(5, &cyclic).len(), 1024, "a cycle must terminate");
    }
}
