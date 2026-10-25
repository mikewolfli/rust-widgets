// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Bridge between `ShortcutManager` and `ActionManager`.
//!
//! `ActionRouter` connects keyboard shortcut events → action commands,
//! completing the event→shortcut→action chain.
//!
//! Connect `ShortcutManager::shortcut_triggered` → `ActionRouter::route_action_id`.

use crate::action::ActionManager;
use crate::action::UndoManager;
use crate::compat::{lock, Mutex, String};
use crate::event::Event;
use crate::shortcut::Key;
use crate::shortcut::Modifiers;
use crate::shortcut::PlatformShortcutStyle;
use crate::shortcut::Shortcut;
use alloc::sync::Arc;

/// Routes keyboard shortcuts to registered actions, bridging `shortcut` and `action` modules.
pub struct ActionRouter<'a> {
    shortcut_mgr: &'a mut crate::shortcut::ShortcutManager,
    action_mgr: &'a mut ActionManager,
}

impl<'a> ActionRouter<'a> {
    /// Creates a new router that connects the two managers.
    pub fn new(
        shortcut_mgr: &'a mut crate::shortcut::ShortcutManager,
        action_mgr: &'a mut ActionManager,
    ) -> Self {
        Self { shortcut_mgr, action_mgr }
    }

    /// Registers an action AND its keyboard shortcut in one step.
    /// Returns `true` if both registration and binding succeeded.
    pub fn connect(
        &mut self,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        text: impl Into<String>,
    ) -> bool {
        let action_id = action_id.into();
        if !self.action_mgr.register_action(action_id.clone(), text) {
            return false;
        }
        // `ShortcutManager` is the single conflict judge: ask it before registering so the reason
        // for a refusal can be reported through the action registry's observation signal.
        let existing = self.shortcut_mgr.get_action(&shortcut).cloned();
        if !self.shortcut_mgr.register(action_id.clone(), shortcut.clone(), "") {
            if let Some(existing) = existing {
                self.action_mgr.report_shortcut_conflict(action_id.clone(), existing);
            }
            self.action_mgr.unregister_action(&action_id);
            return false;
        }
        // Also register in ActionManager's shortcut map
        //
        // NOTE(P4-19): The shortcut is registered in two places
        // intentionally for consistency:
        //   1. `shortcut_mgr.register(...)` — enables shortcut detection
        //      from keyboard input in the shortcut system.
        //   2. `action_mgr.bind_shortcut_type(...)` — enables reverse
        //      lookup (shortcut → action id) inside ActionManager.
        // Both registrations are required for the router to work
        // correctly in both directions.
        //
        // W-08: two writes in one body stay in step only while nothing else writes either table,
        // so the invariant is checked after every successful registration instead of assumed.
        if self.action_mgr.bind_shortcut_type(&shortcut, action_id.clone()) {
            self.assert_tables_reconciled();
            true
        } else {
            self.shortcut_mgr.unregister(&action_id);
            self.action_mgr.unregister_action(&action_id);
            false
        }
    }

    /// The two registration tables this router writes, compared.
    ///
    /// Empty means every registered chord routes to the same action in both tables. A non-empty
    /// result is the W-08 failure mode: something wrote one table without the other.
    pub fn shortcut_reconciliation(
        &self,
    ) -> crate::compat::Vec<crate::action::ShortcutDisagreement> {
        self.action_mgr.shortcut_reconciliation(self.shortcut_mgr)
    }

    /// Debug-build gate run after each successful registration.
    ///
    /// A release build does not pay for the comparison; a debug build fails at the call that
    /// broke the invariant, with the disagreement named, rather than at a later dispatch.
    #[cfg(debug_assertions)]
    fn assert_tables_reconciled(&self) {
        let disagreements = self.shortcut_reconciliation();
        debug_assert!(
            disagreements.is_empty(),
            "ActionRouter left the action and shortcut tables out of sync: {disagreements:?}"
        );
    }

    /// Release-build no-op: see the debug counterpart.
    #[cfg(not(debug_assertions))]
    fn assert_tables_reconciled(&self) {}

    /// Registers an action, shortcut, and callback as one complete binding.
    pub fn connect_callback<F>(
        &mut self,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        text: impl Into<String>,
        callback: F,
    ) -> bool
    where
        F: FnMut() + Send + Sync + 'static,
    {
        let action_id = action_id.into();
        if !self.connect(action_id.clone(), shortcut, text) {
            return false;
        }
        self.action_mgr
            .action(&action_id)
            .map(|action| {
                action.connect_triggered(callback);
                true
            })
            .unwrap_or(false)
    }

    /// Registers standard undo/redo actions using the platform's primary modifier.
    ///
    /// Undo is always `Primary+Z` (`⌘Z` on macOS, `Ctrl+Z` elsewhere). Redo follows
    /// the host convention explicitly: `Primary+Shift+Z` on macOS (the standard
    /// `⇧⌘Z`) and `Primary+Y` on Windows/Linux (`Ctrl+Y`). The physical-Control
    /// spellings remain available through [`Shortcut::ctrl`] for code that needs the
    /// Control key on every platform.
    ///
    /// This is the low-level form: it binds two caller-supplied closures. A host that keeps an
    /// [`UndoManager`] uses [`Self::connect_undo_redo_stack`] instead, which binds the same
    /// accelerators to the active context's stack-top operations.
    pub fn connect_undo_redo<U, R>(&mut self, undo: U, redo: R) -> bool
    where
        U: FnMut() + Send + Sync + 'static,
        R: FnMut() + Send + Sync + 'static,
    {
        let undo_ok = self.connect_callback("undo", Shortcut::primary(Key::Z), "Undo", undo);
        let redo_ok = self.connect_callback("redo", Self::redo_shortcut(), "Redo", redo);
        undo_ok && redo_ok
    }

    /// Binds the undo/redo accelerators to an [`UndoManager`]'s active stack.
    ///
    /// This is the W-202 convenience wrapper: the stack owns the command objects, so the host
    /// only says which manager to drive. The manager is shared rather than borrowed because the
    /// signal slots outlive this call, and the lock is held only for the duration of one
    /// undo/redo operation.
    pub fn connect_undo_redo_stack(&mut self, stack: Arc<Mutex<UndoManager>>) -> bool {
        let for_undo = Arc::clone(&stack);
        let for_redo = Arc::clone(&stack);
        self.connect_undo_redo(
            move || {
                lock(&for_undo).undo();
            },
            move || {
                lock(&for_redo).redo();
            },
        )
    }

    /// The platform's conventional redo accelerator.
    ///
    /// Kept as its own helper (and documented here rather than hidden in
    /// [`connect_undo_redo`](Self::connect_undo_redo)) so the convention is an
    /// explicit, testable contract instead of an implicit `Ctrl+Y` that would be
    /// wrong on macOS.
    fn redo_shortcut() -> Shortcut {
        match crate::platform::platform_facts().shortcut_style() {
            PlatformShortcutStyle::Mac => Shortcut::primary_shift(Key::Z),
            PlatformShortcutStyle::Desktop => Shortcut::primary(Key::Y),
        }
    }

    /// Registers a shortcut binding to an existing action.
    pub fn bind_shortcut(&mut self, action_id: impl Into<String>, shortcut: Shortcut) -> bool {
        let action_id = action_id.into();
        if self.action_mgr.action(&action_id).is_none() {
            return false;
        }
        let existing = self.shortcut_mgr.get_action(&shortcut).cloned();
        if !self.shortcut_mgr.register(action_id.clone(), shortcut.clone(), "") {
            if let Some(existing) = existing {
                self.action_mgr.report_shortcut_conflict(action_id.clone(), existing);
            }
            return false;
        }
        if self.action_mgr.bind_shortcut_type(&shortcut, action_id.clone()) {
            self.assert_tables_reconciled();
            return true;
        }
        self.shortcut_mgr.unregister(&action_id);
        false
    }

    /// Routes a triggered shortcut action id to ActionManager.trigger_action().
    /// Designed to be called from `ShortcutManager::shortcut_triggered` signal.
    pub fn route_action_id(&mut self, action_id: &str) -> bool {
        self.action_mgr.trigger_action(action_id)
    }

    /// The one keyboard dispatch path: registry match, then command trigger.
    ///
    /// `None` means no enabled action owns the chord. A match is triggered here rather than left
    /// to the caller, so a host cannot accept a shortcut and forget to run its command — the two
    /// halves of "the accelerator fired" are one call.
    pub fn dispatch_key(&mut self, key: Key, modifiers: Modifiers) -> Option<String> {
        let action_id = self.shortcut_mgr.dispatch_shortcut(key, modifiers)?;
        self.action_mgr.trigger_action(&action_id);
        Some(action_id)
    }

    /// Dispatches a framework event when it is a key press.
    ///
    /// This is the event-loop entry point: [`crate::event::EventLoop`] offers every key press
    /// here before the focused control sees it, so a global accelerator does not depend on the
    /// focus tree. A non-key event is not a shortcut and reports `false`.
    pub fn dispatch_event(&mut self, event: &Event) -> bool {
        let Event::KeyPress { key, modifiers } = event else {
            return false;
        };
        let Some(key) = Key::from_key_code(*key) else {
            return false;
        };
        self.dispatch_key(key, Modifiers::from_event_bits(*modifiers)).is_some()
    }
}
