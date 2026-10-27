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
use crate::compat::{Cell, HashMap, Rc, RefCell, String, Vec};
use crate::core::{ObjectId, Rect};
use crate::signal::Signal1;

/// The key that names a group.
///
/// Three spellings, because a group can be created three ways: a radio button by its
/// caller-chosen `group_id`, a check box by its parent (the reference toolkit's auto-exclusive
/// rule), and a public [`ButtonGroup`] handle by its own name or generated identity. The variants
/// are distinct so an unnamed handle can never silently join — or capture — a group that a caller
/// named.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum GroupKey {
    /// A caller-chosen group identifier.
    ///
    /// A counted reference rather than an owned `String`: the navigation path resolves this key
    /// on every arrow key and every `is_checked` read, and a `String` clone per read was an
    /// allocation where only an identity comparison was needed (G-A09).
    Name(Rc<str>),
    /// Every exclusive control under one parent.
    Parent(ObjectId),
    /// The identity of a [`ButtonGroup`] handle that has no caller-chosen id.
    ///
    /// A separate variant rather than a reserved `Name`: a caller's string and a handle's
    /// generated number can never collide, so an id-less handle cannot silently adopt — or be
    /// adopted by — a named group.
    Handle(ObjectId),
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
    /// Enabled members in visual order, derived from `members` and kept until it changes.
    ///
    /// Arrow navigation used to collect and sort the peer list on every key press. The member
    /// set only changes when a member joins, leaves or reports a new rectangle/enabled flag, so
    /// the sorted list is rebuilt there instead (G-A09).
    sorted: Option<Vec<GroupMember>>,
}

thread_local! {
    // `HashMap::new` is const only under alloc_frugal (BTreeMap); the std profile cannot be const.
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static GROUPS: RefCell<HashMap<GroupKey, Group>> = RefCell::new(HashMap::new());
    /// Which group each member id is currently registered with.
    ///
    /// Kept beside `GROUPS` so the reverse question ("which group is this member in?") has an
    /// answer that does not walk every group. `register` maintains it and `unregister` drops the
    /// entry, so one widget's id maps to at most one group — the invariant the registry relies on
    /// when it notifies a group's listeners.
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static MEMBER_KEYS: RefCell<HashMap<ObjectId, GroupKey>> = RefCell::new(HashMap::new());
    /// A group handle's explicit membership: the adopted group plus the caller-assigned numeric id.
    ///
    /// This is what lets a [`ButtonGroup`] adopt a control whose own `auto_exclusive`/`group_id`
    /// properties say nothing about the handle. The member's effective key is resolved through
    /// this map first, so an adopted control behaves exactly like one that joined by name.
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static MEMBERSHIP: RefCell<HashMap<ObjectId, GroupMembership>> = RefCell::new(HashMap::new());
    /// The group-level signals, created by a [`ButtonGroup`] handle.
    ///
    /// A group with no handle has no entry, so the common auto-exclusive case pays nothing: the
    /// notification path looks the hub up and returns when there is none.
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static HUBS: RefCell<HashMap<GroupKey, GroupSignals>> = RefCell::new(HashMap::new());
    /// Source of the identities given to id-less [`ButtonGroup`] handles.
    static NEXT_HANDLE: Cell<ObjectId> = const { Cell::new(1) };
}

/// The group a handle has adopted a member into, with the numeric id it answers to there.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GroupMembership {
    key: GroupKey,
    /// The caller-assigned numeric id; defaults to the member's own widget id.
    id: u64,
}

/// The four actions a group reports about its members.
///
/// One enum rather than four entry points because every member reports the same fact the same
/// way — "this member did X" — and the hub is what decides which signal X is. A control therefore
/// calls one function per event it already emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemberEvent {
    /// A member's press began (or repeated).
    Pressed,
    /// A member's press ended (or repeated).
    Released,
    /// A member was activated.
    Clicked,
    /// A member's latched state changed, to the carried value.
    ///
    /// The bool is part of the event rather than something a subscriber has to read back:
    /// the group is the only witness of the change for an unmounted member, so reporting
    /// just the id left every listener querying state it had already been told about.
    Toggled(bool),
}

/// A group's group-level signals.
///
/// `Clone` shares the underlying signal instances, which is what lets a [`ButtonGroup`] handle and
/// the thread-local hub hold the same four signals: the handle subscribes, the members emit, and
/// no member needs a reference to the handle.
#[derive(Clone, Default)]
struct GroupSignals {
    pressed: Signal1<u64>,
    released: Signal1<u64>,
    clicked: Signal1<u64>,
    /// The member's id and its new checked state.
    toggled: Signal1<(u64, bool)>,
}

impl GroupSignals {
    fn emit(&self, event: MemberEvent, id: ObjectId) {
        match event {
            MemberEvent::Pressed => self.pressed.emit(id),
            MemberEvent::Released => self.released.emit(id),
            MemberEvent::Clicked => self.clicked.emit(id),
            MemberEvent::Toggled(checked) => self.toggled.emit((id, checked)),
        }
    }
}

/// Adds `member` to the group named by `key`, or refreshes its recorded facts.
///
/// Called from every state-changing entry point of a member (construction-then-grouping, a
/// geometry change, a click, a focus change). An upsert rather than an error on a repeat: a
/// control cannot know whether it has already announced itself, and re-announcing is idempotent.
pub(crate) fn register(key: GroupKey, id: ObjectId, rect: Rect, enabled: bool) {
    let member = GroupMember { id, rect, enabled };
    // A member belongs to exactly one group, so a registration under a new key first leaves the
    // old one. The reverse index is updated before the unregister runs, which is why the two
    // borrows cannot overlap.
    let previous = MEMBER_KEYS
        .try_with(|keys| keys.borrow_mut().insert(id, key.clone()))
        .ok()
        .flatten()
        .filter(|previous| *previous != key);
    if let Some(previous) = previous {
        unregister(&previous, id);
    }
    let _ = GROUPS.try_with(|groups| {
        let mut groups = groups.borrow_mut();
        let group = groups.entry(key).or_default();
        match group.members.iter_mut().find(|existing| existing.id == id) {
            Some(existing) => {
                if *existing != member {
                    *existing = member;
                    group.sorted = None;
                }
            }
            None => {
                group.members.push(member);
                group.sorted = None;
            }
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
        group.sorted = None;
        if group.members.iter().all(|member| member.id != id) && group.selected == Some(id) {
            group.selected = None;
        }
        if group.members.is_empty() {
            groups.remove(key);
        }
    });
    // The reverse index is dropped only when it still points at this group: a member that has
    // already moved elsewhere must not lose the entry describing its new home.
    let _ = MEMBER_KEYS.try_with(|keys| {
        let mut keys = keys.borrow_mut();
        if keys.get(&id) == Some(key) {
            keys.remove(&id);
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

/// The member that follows `id` in visual order, skipping disabled peers.
///
/// Returns `None` when the group has fewer than two enabled members. `Some(id)` means the walk
/// wrapped back to the caller, which the navigation code needs to tell apart from a move.
///
/// The sorted list is cached in the group and rebuilt only when the member set changes, so a
/// held arrow key no longer allocates and sorts on every repeat (G-A09).
pub(crate) fn next_enabled_member(key: &GroupKey, id: ObjectId, forward: bool) -> Option<ObjectId> {
    GROUPS
        .try_with(|groups| {
            let mut groups = groups.borrow_mut();
            let group = groups.get_mut(key)?;
            if group.sorted.is_none() {
                let mut sorted: Vec<GroupMember> =
                    group.members.iter().copied().filter(|member| member.enabled).collect();
                sorted.sort_by_key(|member| (member.rect.y, member.rect.x));
                group.sorted = Some(sorted);
            }
            let sorted = group.sorted.as_ref()?;
            if sorted.len() < 2 {
                return None;
            }
            let index = sorted.iter().position(|member| member.id == id).unwrap_or(0);
            let next = if forward {
                (index + 1) % sorted.len()
            } else {
                (index + sorted.len() - 1) % sorted.len()
            };
            Some(sorted[next].id)
        })
        .ok()
        .flatten()
}

/// Whether `id` is one of the group's members.
///
/// A borrowed walk rather than [`members`]' clone: `contains` is asked on every selection and
/// does not need a snapshot (G-A10).
pub(crate) fn contains_member(key: &GroupKey, id: ObjectId) -> bool {
    GROUPS
        .try_with(|groups| {
            groups
                .borrow()
                .get(key)
                .is_some_and(|group| group.members.iter().any(|member| member.id == id))
        })
        .unwrap_or(false)
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

/// The group a `ButtonGroup` handle has adopted `id` into, if any.
///
/// Every control resolves its effective group through this first, so a member added through
/// [`ButtonGroup::add_button`] behaves exactly like one that joined by `group_id` or by being an
/// auto-exclusive sibling: the group owns the selection and the member reads it back.
pub(crate) fn membership(id: ObjectId) -> Option<GroupKey> {
    MEMBERSHIP.try_with(|m| m.borrow().get(&id).map(|entry| entry.key.clone())).ok().flatten()
}

/// Reports one of a member's actions to the group it currently belongs to.
///
/// A group with no [`ButtonGroup`] handle has no hub, so this returns without allocating anything:
/// the auto-exclusive-only case pays one map lookup per reported event and nothing else. The hub
/// is cloned **before** emitting, so a listener that re-enters the registry cannot alias the
/// borrow the lookup held.
pub(crate) fn notify_member_event(id: ObjectId, event: MemberEvent) {
    let Some(key) = MEMBER_KEYS
        .try_with(|keys| keys.borrow().get(&id).cloned())
        .ok()
        .flatten()
        .or_else(|| membership(id))
    else {
        return;
    };
    let hub = HUBS.try_with(|hubs| hubs.borrow().get(&key).cloned()).ok().flatten();
    if let Some(hub) = hub {
        hub.emit(event, id);
    }
}

/// Detaches `id` from every group: handle membership, the reverse index and the group itself.
///
/// Called from each control's `Drop`, so a control that is destroyed cannot leave a stale member
/// behind for a peer's navigation to find.
pub(crate) fn leave_all(id: ObjectId) {
    let _ = MEMBERSHIP.try_with(|m| m.borrow_mut().remove(&id));
    let key = MEMBER_KEYS.try_with(|keys| keys.borrow_mut().remove(&id)).ok().flatten();
    if let Some(key) = key {
        // Removed from the reverse index already, so `unregister` only touches the group itself.
        unregister(&key, id);
    }
}

/// The group-level signals of one group, cloned out of the hub.
fn hub_for(key: &GroupKey) -> GroupSignals {
    HUBS.try_with(|hubs| hubs.borrow_mut().entry(key.clone()).or_default().clone())
        .unwrap_or_default()
}

/// Allocates the identity a [`ButtonGroup`] handle uses when it has no caller-chosen id.
///
/// The number is only ever compared inside [`GroupKey::Handle`], so it cannot collide with a
/// caller's name or with a widget id used as a parent key. A thread whose TLS is already being
/// torn down still gets a value rather than a panic; it can only happen while the thread is going
/// away.
fn next_handle_identity() -> ObjectId {
    NEXT_HANDLE
        .try_with(|next| {
            let value = next.get();
            next.set(value.saturating_add(1));
            value
        })
        .unwrap_or(1)
}

/// Moves one group — members, selection, reverse index and signal hub — to `new_key`.
///
/// Used by [`ButtonGroup::set_id`]. When the destination already holds a group, the two are
/// merged: members keep their registration order, the destination's selection wins because it is
/// the group the caller named, and the old key disappears. The signal hub moves only when the
/// destination has none, so an existing handle's subscribers are never silently replaced by a
/// rename.
fn move_group(old_key: &GroupKey, new_key: &GroupKey) {
    if old_key == new_key {
        return;
    }
    let moved = GROUPS.try_with(|groups| {
        let mut groups = groups.borrow_mut();
        groups.remove(old_key)
    });
    if let Some(moved) = moved.ok().flatten() {
        let _ = GROUPS.try_with(|groups| {
            let mut groups = groups.borrow_mut();
            let destination = groups.entry(new_key.clone()).or_default();
            for member in moved.members {
                match destination.members.iter_mut().find(|existing| existing.id == member.id) {
                    Some(existing) => *existing = member,
                    None => destination.members.push(member),
                }
            }
            destination.sorted = None;
            if destination.selected.is_none() {
                destination.selected = moved.selected;
            }
        });
    }
    let moved_hub = HUBS.try_with(|hubs| {
        let mut hubs = hubs.borrow_mut();
        if hubs.contains_key(new_key) {
            // The destination already has a hub; its subscribers win, and the old hub is dropped
            // with the old key.
            let _ = hubs.remove(old_key);
        } else if let Some(hub) = hubs.remove(old_key) {
            // Move the hub rather than recreating it: the handle's subscribers are connected to
            // these signal instances, so replacing them would silently silence the group.
            let _ = hubs.insert(new_key.clone(), hub);
        }
    });
    let _ = moved_hub;
    let _ = MEMBERSHIP.try_with(|m| {
        for entry in m.borrow_mut().values_mut() {
            if entry.key == *old_key {
                entry.key = new_key.clone();
            }
        }
    });
    let _ = MEMBER_KEYS.try_with(|keys| {
        for key in keys.borrow_mut().values_mut() {
            if *key == *old_key {
                *key = new_key.clone();
            }
        }
    });
}

/// The facts the registry needs to order and notify a member: rectangle, enabled, checked.
///
/// Read through the widget registry, which is the only place an id resolves to a control. Under
/// `alloc_frugal` there is no registry, so a member is recorded with a zero rectangle and reported
/// enabled: it corrects the snapshot on its own next event, which is the documented limit of a
/// build with no widget tree.
#[cfg(not(alloc_frugal))]
fn member_facts(id: ObjectId) -> Option<(Rect, bool, bool)> {
    use crate::widget::capability::properties_trait::widget_property_get;
    use crate::widget::capability::types::CapabilityValue;
    crate::widget::runtime::with_widget_mut(id, |widget| {
        let checked =
            matches!(widget_property_get(widget, "checked"), Ok(CapabilityValue::Bool(true)));
        (widget.geometry(), widget.is_enabled(), checked)
    })
}

#[cfg(alloc_frugal)]
fn member_facts(_id: ObjectId) -> Option<(Rect, bool, bool)> {
    Some((Rect::new(0, 0, 0, 0), true, false))
}

/// A handle to one exclusive button group.
///
/// # What this adds over `auto_exclusive` / `group_id`
///
/// Those properties make controls exclude each other; they give the application no way to *ask*
/// which member is selected, to add or remove a member, or to observe a member's action at the
/// group level. This handle is that surface: it is the reference toolkit's `QButtonGroup`, built
/// on the registry this module already keeps rather than on a second one.
///
/// # Identity
///
/// A group is named either by a caller-chosen [`ButtonGroup::set_id`] or by the handle's own
/// generated identity. A named group is the same group a radio button joins through its `group_id`
/// property, so a handle created with [`ButtonGroup::with_id`] immediately sees the controls that
/// already form that group. Adding a member through [`ButtonGroup::add_button`] additionally makes
/// the handle own the widget's grouping regardless of its own properties.
///
/// # Signals
///
/// The four group-level signals carry the member's widget id as their payload, which is the only
/// handle this layer has on a control. `button_toggled` additionally carries the member's new
/// checked state, so a subscriber does not have to query the member back. They are emitted **in
/// addition to** the member's own signals, never instead of them, so an existing per-control
/// subscription keeps working.
///
/// # Cost
///
/// Signals live in a thread-local hub created on first use, so a group that no handle observes
/// allocates nothing and reports nothing. Create one handle and keep it; creating a handle per
/// frame would keep one hub per group alive for the thread's lifetime.
#[derive(Clone)]
pub struct ButtonGroup {
    key: GroupKey,
    signals: GroupSignals,
}

impl Default for ButtonGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl ButtonGroup {
    /// Creates a group with a generated identity.
    pub fn new() -> Self {
        let key = GroupKey::Handle(next_handle_identity());
        Self { signals: hub_for(&key), key }
    }

    /// Creates (or adopts) the group named `id`.
    ///
    /// Adopting rather than replacing is what makes two handles for one name observe the same
    /// group: the second one shares the first one's signals and members.
    pub fn with_id(id: impl Into<String>) -> Self {
        let name: String = id.into();
        let key = GroupKey::Name(Rc::from(name.as_str()));
        Self { signals: hub_for(&key), key }
    }

    /// The group's caller-chosen name, or `None` for a generated identity.
    pub fn id(&self) -> Option<&str> {
        match &self.key {
            GroupKey::Name(name) => Some(&**name),
            GroupKey::Parent(_) | GroupKey::Handle(_) => None,
        }
    }

    /// Renames the group to `id`, moving its members, selection and signals with it.
    ///
    /// A rename onto an existing group merges the two, which is what makes "give this anonymous
    /// group the name the radio buttons already use" mean what it says rather than silently
    /// producing two groups under one name.
    pub fn set_id(&mut self, id: impl Into<String>) {
        let name: String = id.into();
        let new_key = GroupKey::Name(Rc::from(name.as_str()));
        if new_key == self.key {
            return;
        }
        move_group(&self.key, &new_key);
        self.key = new_key;
        self.signals = hub_for(&self.key);
    }

    /// Removes the caller-chosen name, leaving the group under a fresh generated identity.
    pub fn clear_id(&mut self) {
        let new_key = GroupKey::Handle(next_handle_identity());
        move_group(&self.key, &new_key);
        self.key = new_key;
        self.signals = hub_for(&self.key);
    }

    /// Adds `button` to the group, reporting whether the group now owns it.
    ///
    /// The member is registered with the facts the widget registry reports — geometry, enabled
    /// state and checked state — so arrow navigation orders it correctly without waiting for its
    /// first event. A member that was already checked becomes the group's selected member, which
    /// is the same rule a radio button follows when it joins a group already holding a selection.
    ///
    /// A checkable member (check box, radio button, toggle button) keeps the selection current
    /// through its own `set_checked`; a push button has no latch, so its default emphasis still
    /// follows `auto_exclusive` rather than membership. Call [`ButtonGroup::set_checked_button`]
    /// to move a button group's selection programmatically.
    pub fn add_button(&mut self, button: ObjectId) -> bool {
        self.add_button_with_id(button, button)
    }

    /// Adds `button` under a caller-chosen numeric id.
    ///
    /// The numeric id is what [`ButtonGroup::checked_id`] reports, which is the reference
    /// toolkit's `QButtonGroup::addButton(button, id)`. Without it a member's id is its own widget
    /// id, so the two accessors agree by default.
    pub fn add_button_with_id(&mut self, button: ObjectId, id: u64) -> bool {
        // One widget belongs to one group: registering it here leaves whatever group it was in.
        if let Some(previous) =
            MEMBER_KEYS.try_with(|keys| keys.borrow().get(&button).cloned()).ok().flatten()
        {
            if previous != self.key {
                unregister(&previous, button);
            }
        }
        let _ = MEMBERSHIP.try_with(|m| {
            let _ = m.borrow_mut().insert(button, GroupMembership { key: self.key.clone(), id });
        });
        // Ensure the hub exists even for a group created before any member joined, so a control
        // that reports an event before `add_button` returns is still observed.
        let _ = hub_for(&self.key);
        if let Some((rect, enabled, checked)) = member_facts(button) {
            register(self.key.clone(), button, rect, enabled);
            if checked {
                select(&self.key, button);
            }
        }
        true
    }

    /// Removes `button` from the group, reporting whether it was a member.
    pub fn remove_button(&mut self, button: ObjectId) -> bool {
        let in_handle = MEMBERSHIP
            .try_with(|m| m.borrow().get(&button).is_some_and(|entry| entry.key == self.key))
            .unwrap_or(false);
        if in_handle {
            let _ = MEMBERSHIP.try_with(|m| m.borrow_mut().remove(&button));
        }
        let registered = MEMBER_KEYS
            .try_with(|keys| keys.borrow().get(&button) == Some(&self.key))
            .unwrap_or(false);
        unregister(&self.key, button);
        in_handle || registered
    }

    /// The group's members, in registration order.
    ///
    /// A member added through [`ButtonGroup::add_button`] while it is not mounted has no registry
    /// entry yet, so it is reported after the registered members, sorted by id. That keeps the
    /// answer honest — the handle does own it — without pretending it has a rectangle it has not
    /// reported.
    pub fn buttons(&self) -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> =
            members(&self.key).into_iter().map(|member| member.id).collect();
        let extra = MEMBERSHIP.try_with(|m| {
            let mut extra: Vec<ObjectId> = m
                .borrow()
                .iter()
                .filter(|(id, entry)| entry.key == self.key && !ids.contains(*id))
                .map(|(id, _)| *id)
                .collect();
            extra.sort_unstable();
            extra
        });
        if let Ok(extra) = extra {
            ids.extend(extra);
        }
        ids
    }

    /// Whether `button` is currently a member of this group.
    pub fn contains(&self, button: ObjectId) -> bool {
        contains_member(&self.key, button) || membership(button) == Some(self.key.clone())
    }

    /// The widget id of the group's selected member, if one has been chosen.
    pub fn checked_button(&self) -> Option<ObjectId> {
        selected(&self.key)
    }

    /// The numeric id of the group's selected member, if one has been chosen.
    ///
    /// A member added without an explicit id answers with its own widget id, so `checked_id` and
    /// `checked_button` describe the same member until a caller assigns a different number.
    pub fn checked_id(&self) -> Option<u64> {
        let selected = selected(&self.key)?;
        MEMBERSHIP
            .try_with(|m| {
                m.borrow()
                    .get(&selected)
                    .filter(|entry| entry.key == self.key)
                    .map(|entry| entry.id)
            })
            .ok()
            .flatten()
            .or(Some(selected))
    }

    /// Makes `button` the group's selected member, reporting the member it replaced.
    ///
    /// The member is notified through the property layer when it is mounted, so a check box or
    /// radio button unchecks itself and emits its own signals. An unmounted member derives the
    /// answer from this registry on its next read.
    pub fn set_checked_button(&mut self, button: ObjectId) -> Option<ObjectId> {
        if !self.contains(button) {
            return None;
        }
        let previous = select(&self.key, button);
        if let Some(previous) = previous.filter(|previous| *previous != button) {
            notify_checked(&[previous], false);
        }
        previous
    }

    /// Emitted when a member is pressed, with the member's widget id.
    pub fn button_pressed(&self) -> &Signal1<u64> {
        &self.signals.pressed
    }

    /// Emitted when a member is released, with the member's widget id.
    pub fn button_released(&self) -> &Signal1<u64> {
        &self.signals.released
    }

    /// Emitted when a member is clicked, with the member's widget id.
    pub fn button_clicked(&self) -> &Signal1<u64> {
        &self.signals.clicked
    }

    /// Emitted when a member's latched state changes, with the member's widget id and its
    /// new checked state.
    pub fn button_toggled(&self) -> &Signal1<(u64, bool)> {
        &self.signals.toggled
    }
}

/// Forgets every group on this thread.
///
/// Exposed for the tests that exercise the registry directly; production code has no reason to
/// clear every group at once, and doing so would silently detach live controls.
#[cfg(test)]
pub(crate) fn clear() {
    let _ = GROUPS.try_with(|groups| groups.borrow_mut().clear());
    let _ = MEMBER_KEYS.try_with(|keys| keys.borrow_mut().clear());
    let _ = MEMBERSHIP.try_with(|m| m.borrow_mut().clear());
    let _ = HUBS.try_with(|hubs| hubs.borrow_mut().clear());
    let _ = NEXT_HANDLE.try_with(|next| next.set(1));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> GroupKey {
        GroupKey::Name(Rc::from(name))
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

    #[test]
    fn navigation_reads_the_cached_visual_order() {
        clear();
        register(key("g"), 1, Rect::new(0, 0, 10, 10), true);
        register(key("g"), 2, Rect::new(0, 20, 10, 10), false);
        register(key("g"), 3, Rect::new(0, 40, 10, 10), true);
        assert_eq!(next_enabled_member(&key("g"), 1, true), Some(3), "disabled peers are skipped");
        assert_eq!(next_enabled_member(&key("g"), 3, true), Some(1), "the walk wraps");
        assert_eq!(next_enabled_member(&key("g"), 1, false), Some(3));
        assert!(contains_member(&key("g"), 2));
        assert!(!contains_member(&key("g"), 9));
    }

    /// B-010: the group's `button_toggled` signal reports the new checked state alongside the
    /// member id, so a subscriber does not have to query the member back.
    #[test]
    fn button_toggled_carries_the_checked_state() {
        use crate::compat::Arc;
        use crate::compat::Mutex;

        clear();
        let mut group = ButtonGroup::with_id("payload");
        group.add_button(7);

        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        group.button_toggled().connect(move |value| {
            // `crate::compat::lock`, not `.lock().unwrap()`: the `mini` profile's mutex guard has
            // no `unwrap` (there is no poisoning to report), so the shared helper is the one form
            // that compiles in every profile.
            crate::compat::lock(&sink).push(*value);
        });

        notify_member_event(7, MemberEvent::Toggled(true));
        notify_member_event(7, MemberEvent::Toggled(false));

        assert_eq!(*crate::compat::lock(&seen), vec![(7, true), (7, false)]);
    }
}
