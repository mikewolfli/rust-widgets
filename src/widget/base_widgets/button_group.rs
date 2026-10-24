// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A thread-local registry of exclusive control groups.
//!
//! # What this is for
//!
//! A radio button, a check box and a tool button all answer one question the same way: when a
//! member of a group becomes selected, the other members of that group are not. The reference
//! toolkit keeps that fact on the *parent* (`QButtonGroup`/auto-exclusive siblings); this crate
//! has no per-parent state store, so the fact lives here instead.
//!
//! # Why a registry rather than a caller-owned handle
//!
//! The row this module closes asks for a click to deselect a control the clicking control does not
//! own. A `&mut` to a peer is not available at that point, so a shared handle would have to carry
//! the peers as raw references. The registry stores only identity and the facts a group needs —
//! selection, geometry and enabled state — and a control derives its own effective checked state
//! from it. No borrow escapes, and a peer that is not mounted still reports the right state
//! because the answer is derived, not copied.
//!
//! # Why thread-local
//!
//! A widget tree is owned by one thread, and the test harness gives every `#[test]` its own
//! thread. Storing the registry per thread is therefore both the cheapest correct choice and the
//! reason groups cannot leak between tests: a group created by one test is not reachable from the
//! next one. Entries are also removed as soon as their last member leaves, so a long-lived thread
//! does not accumulate dead groups.
use crate::compat::{HashMap, RefCell, String, Vec};
use crate::core::{ObjectId, Rect};

/// The key that names a group.
///
/// Three spellings, because the three controls that use this registry group themselves
/// differently: a radio button by its caller-chosen `group_id`, a check box by its parent (the
/// reference toolkit's auto-exclusive rule), and a control with neither by nothing at all.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum GroupKey {
    /// A caller-chosen group identifier.
    Name(String),
    /// Every exclusive control under one parent.
    Parent(ObjectId),
}

/// One member of a group, as the group needs to see it.
///
/// `rect` and `enabled` are a **snapshot**, refreshed by the member on every event it handles and
/// whenever its geometry changes. They exist so arrow-key navigation can order peers without
/// borrowing them: geometry is the only thing that decides "the next control up/down".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GroupMember {
    /// The member's own id (not a runtime registry id).
    pub id: ObjectId,
    /// The member's rectangle, in its parent's coordinates.
    pub rect: Rect,
    /// Whether the member can be selected.
    pub enabled: bool,
}

#[derive(Default)]
struct Group {
    members: Vec<GroupMember>,
    selected: Option<ObjectId>,
}

thread_local! {
    static GROUPS: RefCell<HashMap<GroupKey, Group>> = RefCell::new(HashMap::new());
}

/// Adds `member` to the group named by `key`, or refreshes its recorded facts.
///
/// Called from every state-changing entry point of a member (construction-then-grouping, a
/// geometry change, a click, a focus change). An upsert rather than an error on a repeat: a
/// control cannot know whether it has already announced itself, and re-announcing is idempotent.
pub(crate) fn register(key: GroupKey, id: ObjectId, rect: Rect, enabled: bool) {
    let member = GroupMember { id, rect, enabled };
    let _ = GROUPS.try_with(|groups| {
        let mut groups = groups.borrow_mut();
        let group = groups.entry(key).or_default();
        match group.members.iter_mut().find(|existing| existing.id == id) {
            Some(existing) => *existing = member,
            None => group.members.push(member),
        }
    });
}

/// Removes `id` from the group named by `key`.
///
/// The group itself is dropped when its last member leaves, including its selection, so a stale
/// selection cannot outlive the controls it described.
pub(crate) fn unregister(key: &GroupKey, id: ObjectId) {
    let _ = GROUPS.try_with(|groups| {
        let mut groups = groups.borrow_mut();
        let Some(group) = groups.get_mut(key) else {
            return;
        };
        group.members.retain(|member| member.id != id);
        if group.members.iter().all(|member| member.id != id) && group.selected == Some(id) {
            group.selected = None;
        }
        if group.members.is_empty() {
            groups.remove(key);
        }
    });
}

/// Makes `id` the selected member of the group and returns the previously selected member.
///
/// `Some(id)` is returned only when the selection actually moved; `None` means either the group
/// had no selection or `id` already held it. Callers use that distinction to decide whether a
/// peer needs to be told, so a repeat selection does not walk the group.
pub(crate) fn select(key: &GroupKey, id: ObjectId) -> Option<ObjectId> {
    GROUPS
        .try_with(|groups| {
            let mut groups = groups.borrow_mut();
            let group = groups.entry(key.clone()).or_default();
            if group.selected == Some(id) {
                return None;
            }
            let previous = group.selected.replace(id);
            previous.filter(|previous| *previous != id)
        })
        .ok()
        .flatten()
}

/// Clears the group's selection, returning the member that held it.
pub(crate) fn clear_selection(key: &GroupKey) -> Option<ObjectId> {
    GROUPS
        .try_with(|groups| groups.borrow_mut().get_mut(key).and_then(|group| group.selected.take()))
        .ok()
        .flatten()
}

/// The selected member of the group, if one has been chosen.
pub(crate) fn selected(key: &GroupKey) -> Option<ObjectId> {
    GROUPS
        .try_with(|groups| groups.borrow().get(key).and_then(|group| group.selected))
        .ok()
        .flatten()
}

/// Whether `id` is the group's selected member.
pub(crate) fn is_selected(key: &GroupKey, id: ObjectId) -> bool {
    selected(key) == Some(id)
}

/// A snapshot of the group's members, in registration order.
///
/// Registration order is not layout order; a caller that needs the visual order sorts by the
/// member rectangles, which is the same rule the reference toolkit's arrow navigation follows.
pub(crate) fn members(key: &GroupKey) -> Vec<GroupMember> {
    GROUPS
        .try_with(|groups| {
            groups.borrow().get(key).map(|group| group.members.clone()).unwrap_or_default()
        })
        .unwrap_or_default()
}

/// Tells the mounted controls in `ids` that their checked property is now `checked`.
///
/// # Why a property write rather than a direct method call
///
/// The only handle the registry has on a peer is its id, and the widget tree is where that id
/// resolves to a `&mut dyn Widget`. Writing through the shared property layer is the crate's
/// existing way to mutate a control it does not statically know, and it means a check box and a
/// radio button are notified by the same line.
///
/// A control that is not mounted is left alone: its checked state is *derived* from this registry,
/// so it already reports the new answer. Only its signal is deferred until it is next driven, which
/// is the honest limit of a control that has no event loop yet.
#[cfg(not(alloc_frugal))]
pub(crate) fn notify_checked(ids: &[ObjectId], checked: bool) {
    use crate::widget::capability::properties_trait::widget_property_set;
    use crate::widget::capability::types::CapabilityValue;
    for id in ids {
        let _ = crate::widget::runtime::with_widget_mut(*id, |widget| {
            let _ = widget_property_set(widget, "checked", CapabilityValue::Bool(checked));
        });
    }
}

/// The profile without a widget registry has no mounted peer to reach, so the notification is a
/// no-op there. The peer's checked state is still derived from this registry, so only its signal
/// is deferred — which is the documented limit of an unmounted control.
#[cfg(alloc_frugal)]
pub(crate) fn notify_checked(_ids: &[ObjectId], _checked: bool) {}

/// Forgets every group on this thread.
///
/// Exposed for the tests that exercise the registry directly; production code has no reason to
/// clear every group at once, and doing so would silently detach live controls.
#[cfg(test)]
pub(crate) fn clear() {
    let _ = GROUPS.try_with(|groups| groups.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> GroupKey {
        GroupKey::Name(name.into())
    }

    #[test]
    fn a_group_selects_one_member_at_a_time() {
        clear();
        register(key("g"), 1, Rect::new(0, 0, 10, 10), true);
        register(key("g"), 2, Rect::new(0, 20, 10, 10), true);

        assert_eq!(select(&key("g"), 1), None, "the first selection has no predecessor");
        assert_eq!(select(&key("g"), 1), None, "a repeat selection is not a move");
        assert_eq!(select(&key("g"), 2), Some(1), "the previous member is reported once");
        assert!(is_selected(&key("g"), 2));
        assert!(!is_selected(&key("g"), 1));
    }

    #[test]
    fn the_group_disappears_when_its_last_member_leaves() {
        clear();
        register(key("g"), 7, Rect::new(0, 0, 4, 4), true);
        select(&key("g"), 7);
        unregister(&key("g"), 7);

        assert_eq!(selected(&key("g")), None);
        assert!(members(&key("g")).is_empty());
    }

    #[test]
    fn a_geometry_refresh_replaces_the_snapshot() {
        clear();
        register(key("g"), 3, Rect::new(0, 0, 10, 10), true);
        register(key("g"), 3, Rect::new(40, 50, 10, 10), false);

        let members = members(&key("g"));
        assert_eq!(members.len(), 1, "a refresh must not duplicate the member");
        assert_eq!(members[0].rect, Rect::new(40, 50, 10, 10));
        assert!(!members[0].enabled);
    }

    #[test]
    fn parent_and_name_groups_are_distinct() {
        clear();
        register(GroupKey::Parent(1), 1, Rect::new(0, 0, 1, 1), true);
        register(key("1"), 2, Rect::new(0, 0, 1, 1), true);
        select(&key("1"), 2);
        assert!(!is_selected(&GroupKey::Parent(1), 2));
    }
}
