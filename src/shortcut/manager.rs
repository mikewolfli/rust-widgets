// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{
    control_scope_key, Key, Modifiers, PlatformShortcutStyle, Shortcut, ShortcutEntry,
    ShortcutScope,
};
use crate::compat::{format, Arc, Box, HashMap, String, Vec};
use crate::core::ObjectId;
use crate::event::Event;
use crate::signal::Signal1;
/// Global shortcut manager for registering and dispatching shortcuts.
///
/// # Scopes (W-201)
///
/// Bindings registered with [`ShortcutManager::register`] live in the **global** scope and behave
/// exactly as before. [`ShortcutManager::register_in`] attaches a binding to a named context (a
/// window or modal domain), and [`ShortcutManager::push_scope`] / `push_modal_scope` make contexts
/// active. Resolution walks the active stack from the top down and falls back to the global scope,
/// so the same chord can mean different commands in different windows and a modal scope can shield
/// the layers below it.
pub struct ShortcutManager {
    /// Map from shortcut to action ID for the **global** scope.
    ///
    /// Kept as its own map because it is the historical public view
    /// ([`Self::all_shortcuts`]); scoped bindings live in `scoped_shortcuts` so the two cannot
    /// collide by accident.
    shortcuts: HashMap<Shortcut, String>,
    /// Map from `(scope id, shortcut)` to action ID for scoped bindings.
    scoped_shortcuts: HashMap<(String, Shortcut), String>,
    /// Map from action ID to entry.
    entries: HashMap<String, ShortcutEntry>,
    /// W-P01: candidate index from `(main key, normalized event modifier set)` to the bindings
    /// that can answer to that chord, in registration order.
    ///
    /// The index only *narrows candidates*: every candidate is still judged by
    /// [`Self::matches_binding`], the single matching rule, so the runtime platform style keeps
    /// the final word. A `primary` binding is indexed under the desktop lift bucket too, and a
    /// platform whose style declines the lift simply rejects that candidate at match time.
    index: HashMap<(Key, Modifiers), Vec<IndexedBinding>>,
    /// Active scope stack, bottom first. Resolution walks it in reverse (top first).
    scopes: Vec<ActiveScope>,
    /// The host's focus chain, outermost last, when a control-scoped binding is in use (W-12).
    ///
    /// `None` (the default) means no control scope resolves, which is the behaviour before control
    /// scopes existed. The closure is supplied by the host rather than read from the widget
    /// runtime: `shortcut` has no access to the focus tree (and the runtime is compiled out of the
    /// stripped profiles), so the host states the fact instead of the manager guessing.
    focus_chain: Option<Arc<dyn Fn() -> Vec<ObjectId> + Send + Sync>>,
    /// Emitted when a shortcut is triggered.
    pub shortcut_triggered: Signal1<String>,
    /// Emitted when a shortcut conflict is detected.
    pub conflict_detected: Signal1<(Shortcut, String, String)>,
}

/// One active context on the scope stack.
#[derive(Debug, Clone)]
struct ActiveScope {
    /// Host-chosen context name (a window id, a modal dialog id, …).
    id: String,
    /// Whether this scope shields every scope below it, modal-dialog style.
    modal: bool,
}

/// One entry of the chord index: the facts `lookup` needs before the matching rule runs.
///
/// `shortcut` mirrors the authoritative copy in `entries`; the index is a derived structure
/// maintained only by [`ShortcutManager::register_scoped`], [`ShortcutManager::remove_binding`],
/// [`ShortcutManager::unregister_shortcut`] and [`ShortcutManager::clear`], so the copy cannot
/// drift from the entry it mirrors.
#[derive(Clone)]
struct IndexedBinding {
    /// `None` for a global binding, the owning scope's id otherwise.
    scope: Option<String>,
    /// The action the chord routes to.
    action_id: String,
    /// Copy of the registered binding, handed to `matches_binding` unchanged.
    shortcut: Shortcut,
}

impl ShortcutManager {
    /// Creates a new shortcut manager.
    pub fn new() -> Self {
        Self {
            shortcuts: HashMap::new(),
            scoped_shortcuts: HashMap::new(),
            entries: HashMap::new(),
            index: HashMap::new(),
            scopes: Vec::new(),
            focus_chain: None,
            shortcut_triggered: Signal1::new(),
            conflict_detected: Signal1::new(),
        }
    }
    /// Registers a new shortcut in the global scope.
    /// Returns true if registration succeeded, false if there was a conflict.
    pub fn register(
        &mut self,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        description: impl Into<String>,
    ) -> bool {
        self.register_scoped(None, action_id, shortcut, description)
    }

    /// Registers a shortcut bound to one control (W-12).
    ///
    /// The binding resolves only while `control_id` is in the focus chain supplied through
    /// [`Self::set_focus_chain_lookup`], so a control's local chord stops firing the moment focus
    /// leaves it — without the host pushing and popping a scope on every focus change.
    ///
    /// A control scope wins over a window scope and over the global table on the same chord; that
    /// is what makes a local rebinding possible at all. With no focus-chain lookup installed, a
    /// binding registered here never resolves (nothing can claim the control is focused).
    ///
    /// # Why the focus fact is not read here
    ///
    /// `shortcut` is a plain registry: the focus tree lives in the widget runtime, which the
    /// stripped profiles do not compile, and a second source of truth for "what is focused"
    /// would be the drift this module exists to avoid. The host that owns both publishes the
    /// chain once through [`Self::set_focus_chain_lookup`].
    pub fn register_for_control(
        &mut self,
        control_id: ObjectId,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        description: impl Into<String>,
    ) -> bool {
        self.register_scoped(
            ShortcutScope::Control(control_id).scope_key(),
            action_id,
            shortcut,
            description,
        )
    }

    /// Registers a shortcut in `scope` instead of the global scope.
    ///
    /// The binding only fires while `scope` is on the active stack (see [`Self::push_scope`]), and
    /// it shadows a global binding on the same chord. The same chord may be bound in two different
    /// scopes — that is the point of scopes — so conflicts are judged **within** a scope only.
    pub fn register_in(
        &mut self,
        scope: impl Into<String>,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        description: impl Into<String>,
    ) -> bool {
        self.register_scoped(Some(scope.into()), action_id, shortcut, description)
    }

    /// Shared body of [`Self::register`] and [`Self::register_in`].
    fn register_scoped(
        &mut self,
        scope: Option<String>,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        description: impl Into<String>,
    ) -> bool {
        let action_id = action_id.into();
        let description = description.into();
        // Conflict is judged inside the binding's own scope: a global chord and a scoped chord may
        // share a key, which is how a modal dialog rebinds Enter without fighting the main window.
        let conflict = match &scope {
            None => {
                self.shortcuts.get(&shortcut).filter(|existing| **existing != action_id).cloned()
            }
            Some(scope) => self
                .scoped_shortcuts
                .get(&(scope.clone(), shortcut.clone()))
                .filter(|existing| **existing != action_id)
                .cloned(),
        };
        if let Some(existing_action) = conflict {
            self.conflict_detected.emit((shortcut.clone(), action_id.clone(), existing_action));
            return false;
        }
        // One chord per action: re-registering an id moves its binding rather than adding one.
        if let Some(entry) = self.entries.remove(&action_id) {
            self.remove_binding(&entry);
        }
        let entry = ShortcutEntry {
            action_id: action_id.clone(),
            description,
            shortcut: shortcut.clone(),
            enabled: true,
            scope: scope.clone(),
        };
        let indexed = IndexedBinding {
            scope: scope.clone(),
            action_id: action_id.clone(),
            shortcut: shortcut.clone(),
        };
        match scope {
            Some(scope) => {
                self.scoped_shortcuts.insert((scope, shortcut), action_id.clone());
            }
            None => {
                self.shortcuts.insert(shortcut, action_id.clone());
            }
        }
        self.entries.insert(action_id, entry);
        self.index_binding(indexed);
        true
    }

    /// Removes `entry`'s chord from the table its scope belongs to.
    ///
    /// # Only the binding that still owns the chord is removed (W-02)
    ///
    /// The table removal is guarded by the action id the table currently routes to. An entry can
    /// outlive its table row only in a race or a stale copy; removing the row unconditionally then
    /// deleted whichever binding holds that chord *now* — the exact "a chord removal takes a
    /// different action's binding with it" failure. The guard makes the removal exact (scope +
    /// chord + action) and leaves every other binding of the same chord alone.
    fn remove_binding(&mut self, entry: &ShortcutEntry) {
        match &entry.scope {
            Some(scope) => {
                let key = (scope.clone(), entry.shortcut.clone());
                if self.scoped_shortcuts.get(&key).is_some_and(|action| action == &entry.action_id)
                {
                    self.scoped_shortcuts.remove(&key);
                }
            }
            None => {
                if self
                    .shortcuts
                    .get(&entry.shortcut)
                    .is_some_and(|action| action == &entry.action_id)
                {
                    self.shortcuts.remove(&entry.shortcut);
                }
            }
        }
        self.remove_indexed(entry.scope.as_deref(), &entry.shortcut, &entry.action_id);
    }

    /// Adds one binding to every index bucket it can answer from (W-P01).
    ///
    /// A binding is indexed under its own modifier set. A `primary` binding is additionally
    /// indexed under the desktop lift (`modifiers - PRIMARY + CTRL`), because that is the other
    /// normalized event `matches_binding` can answer yes from. The extra bucket is a candidate
    /// list only: on a platform whose style declines the lift, `matches_binding` rejects the
    /// candidate exactly as the linear scan did.
    fn index_binding(&mut self, binding: IndexedBinding) {
        let direct = (binding.shortcut.key, binding.shortcut.modifiers);
        self.index.entry(direct).or_default().push(binding.clone());
        if binding.shortcut.modifiers.contains(Modifiers::PRIMARY) {
            let lifted = (
                binding.shortcut.key,
                binding.shortcut.modifiers.without(Modifiers::PRIMARY) | Modifiers::CTRL,
            );
            self.index.entry(lifted).or_default().push(binding);
        }
    }

    /// Removes one binding from every bucket [`Self::index_binding`] may have placed it in.
    fn remove_indexed(&mut self, scope: Option<&str>, shortcut: &Shortcut, action_id: &str) {
        Self::remove_from_bucket(
            &mut self.index,
            (shortcut.key, shortcut.modifiers),
            scope,
            action_id,
        );
        if shortcut.modifiers.contains(Modifiers::PRIMARY) {
            let lifted =
                (shortcut.key, shortcut.modifiers.without(Modifiers::PRIMARY) | Modifiers::CTRL);
            Self::remove_from_bucket(&mut self.index, lifted, scope, action_id);
        }
    }

    /// Drops one `(scope, action)` pair from a bucket, removing the bucket when it empties so a
    /// churning registry cannot accumulate empty vectors.
    fn remove_from_bucket(
        index: &mut HashMap<(Key, Modifiers), Vec<IndexedBinding>>,
        key: (Key, Modifiers),
        scope: Option<&str>,
        action_id: &str,
    ) {
        let Some(bucket) = index.get_mut(&key) else {
            return;
        };
        bucket.retain(|binding| {
            !(binding.action_id == action_id && binding.scope.as_deref() == scope)
        });
        if bucket.is_empty() {
            index.remove(&key);
        }
    }

    /// Unregisters a shortcut by action ID.
    pub fn unregister(&mut self, action_id: &str) -> bool {
        if let Some(entry) = self.entries.remove(action_id) {
            self.remove_binding(&entry);
            true
        } else {
            false
        }
    }
    /// Unregisters a shortcut by key combination (the **global** binding only).
    ///
    /// # Scope (W-02)
    ///
    /// A chord is not unique across scopes: the same chord may be global, bound in a window scope
    /// and bound for a control. This call removes the **global** binding and nothing else. Use
    /// [`Self::unregister_shortcut_in`] for a window scope and
    /// [`Self::unregister_for_control`] for a control scope.
    ///
    /// The entry is removed only when it still describes this exact binding (no scope, this chord),
    /// so a removal can never delete a scoped action's entry — which would leave that scope's
    /// table row pointing at an action id with no entry, i.e. a binding that answers the chord but
    /// resolves to nothing. Returns `false` when the global table has no such chord.
    pub fn unregister_shortcut(&mut self, shortcut: &Shortcut) -> bool {
        let Some(action_id) = self.shortcuts.get(shortcut).cloned() else {
            return false;
        };
        self.shortcuts.remove(shortcut);
        let owns_entry = self
            .entries
            .get(&action_id)
            .is_some_and(|entry| entry.scope.is_none() && &entry.shortcut == shortcut);
        if owns_entry {
            self.entries.remove(&action_id);
        }
        self.remove_indexed(None, shortcut, &action_id);
        true
    }

    /// Unregisters `shortcut` from exactly one named scope (W-02).
    ///
    /// The `(scope, chord)` pair is matched exactly — a chord bound in another scope, or globally,
    /// is untouched. The action entry is removed only if it still describes this binding, so the
    /// removal cannot orphan another action's table row.
    ///
    /// Returns `false` when the scope does not hold that chord.
    pub fn unregister_shortcut_in(
        &mut self,
        scope: impl Into<String>,
        shortcut: &Shortcut,
    ) -> bool {
        let scope = scope.into();
        let key = (scope.clone(), shortcut.clone());
        let Some(action_id) = self.scoped_shortcuts.remove(&key) else {
            return false;
        };
        let owns_entry = self.entries.get(&action_id).is_some_and(|entry| {
            entry.scope.as_deref() == Some(scope.as_str()) && &entry.shortcut == shortcut
        });
        if owns_entry {
            self.entries.remove(&action_id);
        }
        self.remove_indexed(Some(scope.as_str()), shortcut, &action_id);
        true
    }

    /// Unregisters `shortcut` from the scope of `control_id` (W-02, W-12).
    ///
    /// Convenience over [`Self::unregister_shortcut_in`] with
    /// [`ShortcutScope::Control`]`(control_id)`'s key, so a caller that registered the binding
    /// through [`Self::register_for_control`] does not have to spell the internal key.
    pub fn unregister_for_control(&mut self, control_id: ObjectId, shortcut: &Shortcut) -> bool {
        self.unregister_shortcut_in(control_scope_key(control_id), shortcut)
    }

    /// Returns whether an action ID is already registered.
    pub fn contains_action(&self, action_id: &str) -> bool {
        self.entries.contains_key(action_id)
    }
    /// Handles a key event and triggers the associated action if a shortcut matches.
    ///
    /// # Matching rule
    ///
    /// A key event's [`Modifiers::PRIMARY`] bit means "the platform's primary
    /// accelerator". The physical Control bit that macOS also reports alongside
    /// Command is **not** part of the command's identity, so it is cleared before
    /// the lookup. On Windows/Linux a Control press is lifted the other way, into
    /// PRIMARY, so `Shortcut::primary` bindings answer to Ctrl as the platform
    /// convention requires. Physical-Control bindings (`Shortcut::ctrl`) are
    /// deliberately left alone on macOS: there Command and Control are different
    /// keys, and collapsing them would fire `⌘C` for a `Control+C` binding.
    pub fn handle_key_event(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let Some(action_id) = self.action_for_key(key, modifiers) else {
            return false;
        };
        self.shortcut_triggered.emit(action_id);
        true
    }

    /// Resolves the action registered for a key press.
    ///
    /// This is the public query the rest of the crate uses instead of re-implementing the
    /// platform modifier rule. `None` means no **enabled** action owns this chord; a disabled
    /// entry is deliberately indistinguishable from an absent one, because dispatching either
    /// would be wrong.
    pub fn action_for_key(&self, key: Key, modifiers: Modifiers) -> Option<String> {
        let action_id = self.lookup(key, modifiers)?;
        let entry = self.entries.get(&action_id)?;
        entry.enabled.then_some(action_id)
    }

    /// Whether a typed binding matches a key press, using the same rule as `Self::lookup`.
    ///
    /// # Why this is the single decision point
    ///
    /// Three callers need the answer — the global registry, a menu item's registered binding,
    /// and a standalone `Action` widget — and they used to each carry a copy of the platform
    /// normalisation. Copies drift, so a binding could fire on one path and not another. This
    /// function is the one implementation; the others delegate to it.
    pub fn matches_binding(shortcut: &Shortcut, key: Key, modifiers: Modifiers) -> bool {
        if shortcut.key != key {
            return false;
        }
        let style = crate::platform::platform_facts().shortcut_style();
        let normalized = Self::normalize_event_modifiers(modifiers, style);
        if shortcut.modifiers == normalized {
            return true;
        }
        // Windows/Linux: Control is the platform accelerator, so a `primary` binding answers
        // to a Control press. This is the same lift `lookup` applies to the registry key.
        style == PlatformShortcutStyle::Desktop
            && normalized.contains(Modifiers::CTRL)
            && !normalized.contains(Modifiers::PRIMARY)
            && shortcut.modifiers == normalized.without(Modifiers::CTRL) | Modifiers::PRIMARY
    }

    /// Dispatches a key press through the registry, returning the action id that fired.
    ///
    /// This is the host entry point shared by `Action` widgets and native menu items: a host
    /// calls it once per key press and routes the returned id to [`crate::action::ActionManager`].
    /// Keeping it here means conflict detection and matching see every binding, including the
    /// menu accelerators that used to be handled only inside the menu code.
    pub fn dispatch_shortcut(&mut self, key: Key, modifiers: Modifiers) -> Option<String> {
        let action_id = self.action_for_key(key, modifiers)?;
        self.shortcut_triggered.emit(action_id.clone());
        Some(action_id)
    }

    /// Resolves the action registered for a key press, applying the matching
    /// rule documented on [`ShortcutManager::matches_binding`].
    ///
    /// # Resolution order (W-201, W-12)
    ///
    /// 1. **Control scopes**, in focus-chain order (innermost first). A binding registered for the
    ///    focused control wins over every window scope and over the global table — that is what a
    ///    local rebinding means. With no focus-chain lookup installed this step is skipped, which
    ///    is exactly the behaviour before control scopes existed.
    /// 2. The active scope stack is walked from the top down. A matching binding in a scope wins
    ///    over every lower scope and over the global scope. A **modal** scope terminates the walk
    ///    whether or not it matched, which is what shields the layers below a modal dialog.
    /// 3. After the stack is exhausted, global bindings are consulted exactly as before scopes
    ///    existed.
    ///
    /// The global fallback is still a linear scan over the entries **on purpose**: the hash map is
    /// the reverse index ([`Self::get_action`]), but a second matching path would be a second
    /// rule. The chord index only fetches the candidates for the pressed `(key, normalized
    /// modifiers)`; every candidate is still judged by [`Self::matches_binding`], so registry
    /// dispatch and an unregistered widget action cannot disagree about a chord. When two bindings
    /// in one scope can answer to the same press (a physical-Control binding and a `primary`
    /// binding under the desktop lift), the bucket keeps their registration order, so the winner
    /// is deterministic (W-P01).
    ///
    /// **Reachability of control scopes (W-12):** the focus-chain lookup is installed by the
    /// host, and this crate installs none — `shortcut` cannot read the widget runtime's focus
    /// tree (it is compiled out of the stripped profiles), so claiming a chain here would be a
    /// guess. Without the lookup, a control binding simply never resolves.
    fn lookup(&self, key: Key, modifiers: Modifiers) -> Option<String> {
        // The same normalization `matches_binding` applies, used here only to pick the bucket;
        // acceptance is still decided per candidate by `matches_binding` below.
        let style = crate::platform::platform_facts().shortcut_style();
        let normalized = Self::normalize_event_modifiers(modifiers, style);
        let bucket = self.index.get(&(key, normalized))?;
        // W-12: control scopes first, innermost first. Each candidate is still judged by the one
        // matching rule, so the platform lift behaves identically here and in a window scope.
        if let Some(focus_chain) = self.focus_chain.as_ref() {
            for control in focus_chain() {
                let control_scope = control_scope_key(control);
                let matched = bucket
                    .iter()
                    .find(|binding| {
                        binding.scope.as_deref() == Some(control_scope.as_str())
                            && Self::matches_binding(&binding.shortcut, key, modifiers)
                    })
                    .map(|binding| binding.action_id.clone());
                if matched.is_some() {
                    return matched;
                }
            }
        }
        for active in self.scopes.iter().rev() {
            let matched = bucket
                .iter()
                .find(|binding| {
                    binding.scope.as_deref() == Some(active.id.as_str())
                        && Self::matches_binding(&binding.shortcut, key, modifiers)
                })
                .map(|binding| binding.action_id.clone());
            if matched.is_some() {
                return matched;
            }
            if active.modal {
                return None;
            }
        }
        bucket
            .iter()
            .find(|binding| {
                binding.scope.is_none() && Self::matches_binding(&binding.shortcut, key, modifiers)
            })
            .map(|binding| binding.action_id.clone())
    }

    /// Pushes a non-modal scope onto the active stack.
    ///
    /// Bindings registered in this scope now win over global ones on the same chord. A scope id may
    /// be pushed more than once; the stack then contains it more than once, which is harmless but
    /// rarely useful.
    pub fn push_scope(&mut self, scope: impl Into<String>) {
        self.scopes.push(ActiveScope { id: scope.into(), modal: false });
    }

    /// Installs the host's focus-chain lookup, which is what makes control scopes resolve (W-12).
    ///
    /// The closure returns the currently focused control first, then each ancestor outwards — the
    /// order the focus tree walks. A binding registered through [`Self::register_for_control`] for
    /// a control in that list then wins over window and global bindings on the same chord.
    ///
    /// # What a host has to publish
    ///
    /// Only controls that are actually reachable count: a control behind a modal dialog should not
    /// be in the chain while the modal is up, because its local chords must not fire. The host
    /// owns that fact (it built both trees), so it is stated here rather than guessed.
    ///
    /// # Clearing
    ///
    /// [`Self::clear_focus_chain_lookup`] restores the "no control scope resolves" behaviour,
    /// which is the default and the pre-W-12 behaviour.
    pub fn set_focus_chain_lookup(&mut self, lookup: Box<dyn Fn() -> Vec<ObjectId> + Send + Sync>) {
        self.focus_chain = Some(Arc::from(lookup));
    }

    /// Removes the focus-chain lookup, reporting whether one was installed (W-12).
    pub fn clear_focus_chain_lookup(&mut self) -> bool {
        self.focus_chain.take().is_some()
    }

    /// Whether `control_id` has `shortcut` registered as a control-scoped binding (W-12).
    ///
    /// Says nothing about the focus chain: the binding may exist and not resolve because the
    /// control is not focused, which is the point of a control scope.
    pub fn has_control_shortcut(&self, control_id: ObjectId, shortcut: &Shortcut) -> bool {
        self.has_scoped_shortcut(&control_scope_key(control_id), shortcut)
    }

    /// Pushes a **modal** scope: it wins like any other scope and also stops the lookup from
    /// reaching the scopes and global bindings beneath it.
    pub fn push_modal_scope(&mut self, scope: impl Into<String>) {
        self.scopes.push(ActiveScope { id: scope.into(), modal: true });
    }

    /// Pops the top scope, returning its id.
    pub fn pop_scope(&mut self) -> Option<String> {
        self.scopes.pop().map(|scope| scope.id)
    }

    /// The innermost active scope, or `None` when only the global scope is active.
    pub fn active_scope(&self) -> Option<&str> {
        self.scopes.last().map(|scope| scope.id.as_str())
    }

    /// Whether `scope` is currently on the stack.
    pub fn is_scope_active(&self, scope: &str) -> bool {
        self.scopes.iter().any(|active| active.id == scope)
    }

    /// The active scope ids in activation order (bottom first).
    pub fn active_scopes(&self) -> Vec<String> {
        self.scopes.iter().map(|scope| scope.id.clone()).collect()
    }

    /// Removes every active scope, returning to the global-only behaviour.
    pub fn clear_scopes(&mut self) {
        self.scopes.clear();
    }

    /// Removes the Control bit that macOS reports alongside Command.
    ///
    /// `⌘C` and `⌃C` are different chords, but macOS sets the Control flag on
    /// some Command events; leaving it in place would make `Shortcut::primary`
    /// bindings unreachable. On other platforms the input is already canonical.
    ///
    /// The deciding fact is the backend's shortcut style, passed in by the
    /// caller — this function does not sniff the OS (principle #37).
    fn normalize_event_modifiers(modifiers: Modifiers, style: PlatformShortcutStyle) -> Modifiers {
        if style == PlatformShortcutStyle::Mac
            && modifiers.contains(Modifiers::PRIMARY)
            && modifiers.contains(Modifiers::CTRL)
        {
            return modifiers.without(Modifiers::CTRL);
        }
        modifiers
    }
    /// Handles a framework event and triggers a shortcut when it is a key press.
    ///
    /// This is the host's entry point: no framework event loop calls it. The
    /// application owns its shortcut manager and feeds key presses here from
    /// its own dispatch path (see the module docs for the wiring and why the
    /// framework does not pump this itself).
    pub fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::KeyPress { key, modifiers } = event {
            if let Some(key) = Key::from_key_code(*key) {
                return self.handle_key_event(key, Modifiers::from_event_bits(*modifiers));
            }
        }
        false
    }
    /// Enables a shortcut by action ID.
    pub fn enable(&mut self, action_id: &str) -> bool {
        if let Some(entry) = self.entries.get_mut(action_id) {
            entry.enabled = true;
            true
        } else {
            false
        }
    }
    /// Disables a shortcut by action ID.
    pub fn disable(&mut self, action_id: &str) -> bool {
        if let Some(entry) = self.entries.get_mut(action_id) {
            entry.enabled = false;
            true
        } else {
            false
        }
    }
    /// Returns the shortcut for an action ID, in any scope.
    pub fn get_shortcut(&self, action_id: &str) -> Option<&Shortcut> {
        self.entries.get(action_id).map(|e| &e.shortcut)
    }
    /// Returns the action ID for a **global** shortcut.
    ///
    /// Scoped bindings are not part of this view: the same chord may map to a different action in
    /// each scope, so a scope-free key cannot name one answer.
    pub fn get_action(&self, shortcut: &Shortcut) -> Option<&String> {
        self.shortcuts.get(shortcut)
    }
    /// Returns all registered **global** shortcuts.
    ///
    /// See [`Self::get_action`] for why scoped bindings are excluded.
    pub fn all_shortcuts(&self) -> &HashMap<Shortcut, String> {
        &self.shortcuts
    }
    /// Returns all entries, global and scoped.
    pub fn all_entries(&self) -> &HashMap<String, ShortcutEntry> {
        &self.entries
    }
    /// Clears all shortcuts, scoped or global, and every active scope.
    pub fn clear(&mut self) {
        self.shortcuts.clear();
        self.scoped_shortcuts.clear();
        self.entries.clear();
        self.index.clear();
        self.scopes.clear();
    }
    /// Returns true if a **global** shortcut is registered.
    pub fn has_shortcut(&self, shortcut: &Shortcut) -> bool {
        self.shortcuts.contains_key(shortcut)
    }
    /// Returns true if `scope` has `shortcut` registered.
    pub fn has_scoped_shortcut(&self, scope: &str, shortcut: &Shortcut) -> bool {
        self.scoped_shortcuts.contains_key(&(String::from(scope), shortcut.clone()))
    }
    /// Returns true if an action has a shortcut registered.
    pub fn has_action(&self, action_id: &str) -> bool {
        self.entries.contains_key(action_id)
    }
    /// Finds conflicts between the given shortcut and existing shortcuts.
    pub fn find_conflicts(&self, shortcut: &Shortcut) -> Vec<&ShortcutEntry> {
        self.entries.values().filter(|e| e.shortcut.conflicts_with(shortcut)).collect()
    }
}
crate::impl_default_via_new!(ShortcutManager);

impl Shortcut {
    /// Serializes this binding as a stable, single-key JSON object.
    ///
    /// The value is the same canonical spelling [`Shortcut::format_shortcut`] produces, which
    /// [`Shortcut::from_string`] parses back. A stable single field is enough because a shortcut
    /// has exactly one fact; the field name is the versioning seam, not a second schema.
    pub fn to_json(&self) -> String {
        format!("{{\"shortcut\":\"{}\"}}", json_escape(&self.format_shortcut()))
    }

    /// Reads the binding written by [`Shortcut::to_json`].
    ///
    /// Unknown keys are ignored (an older reader must not fail on a newer document), but a
    /// missing or unparsable `shortcut` is an error rather than a silently empty binding.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let spelling = json_string_field(text, "shortcut")
            .ok_or_else(|| String::from("shortcut JSON has no `shortcut` string field"))?;
        Self::from_string(&spelling)
            .ok_or_else(|| format!("`{spelling}` is not a shortcut this build understands"))
    }
}

/// Escapes the four characters a JSON string may not carry raw.
fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// Reads one flat string field from a single-level JSON object.
///
/// Deliberately small: the caller's schema is flat and versioned by field name, so a full JSON
/// parser would be more machinery than the one fact being read.
fn json_string_field(text: &str, field: &str) -> Option<String> {
    let key = format!("\"{}\"", field);
    let start = text.find(&key)? + key.len();
    let rest = &text[start..];
    let colon = rest.find(':')? + 1;
    let after_colon = rest[colon..].trim_start();
    let mut chars = after_colon.strip_prefix('"')?.chars();
    let mut out = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            other => out.push(other),
        }
    }
    None
}
