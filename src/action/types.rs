// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::{String, Vec};
use crate::core::ObjectId;
use crate::signal::{ConnectionHandle, GenericSignal};
use alloc::sync::Arc;
use core::fmt;
/// Represents a user-invokable command with enabled state and trigger signal.
#[derive(Clone)]
pub struct Action {
    /// Stable action identifier.
    pub id: String,
    /// Human-readable action label.
    pub text: String,
    enabled: bool,
    checkable: bool,
    checked: bool,
    triggered: GenericSignal,
    toggled: crate::signal::Signal1<bool>,
    enabled_changed: crate::signal::Signal1<bool>,
}
impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Action")
            .field("id", &self.id)
            .field("text", &self.text)
            .field("enabled", &self.enabled)
            .field("checkable", &self.checkable)
            .field("checked", &self.checked)
            .field("triggered", &self.triggered.slot_count())
            .field("toggled", &self.toggled.slot_count())
            .field("enabled_changed", &self.enabled_changed.slot_count())
            .finish()
    }
}
impl Action {
    /// Creates a new enabled action with the provided id and label.
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            enabled: true,
            checkable: false,
            checked: false,
            triggered: GenericSignal::new(),
            toggled: crate::signal::Signal1::new(),
            enabled_changed: crate::signal::Signal1::new(),
        }
    }
    /// Returns whether this action can currently be triggered.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    /// Updates whether this action can be triggered.
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled {
            return;
        }
        self.enabled = enabled;
        self.enabled_changed.emit(enabled);
    }
    /// Returns whether action supports checked state.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }
    /// Enables/disables checkable behavior.
    pub fn set_checkable(&mut self, checkable: bool) {
        if self.checkable == checkable {
            return;
        }
        self.checkable = checkable;
        if !checkable {
            self.set_checked(false);
        }
    }
    /// Returns checked state for checkable actions.
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    /// Updates checked state for checkable actions.
    pub fn set_checked(&mut self, checked: bool) {
        let normalized = if self.checkable { checked } else { false };
        if self.checked == normalized {
            return;
        }
        self.checked = normalized;
        self.toggled.emit(self.checked);
    }
    /// Connects a callback that runs when the action is triggered.
    pub fn connect_triggered<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.triggered.connect(slot)
    }
    /// Connects a callback for checked-state changes.
    pub fn connect_toggled<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(Arc<bool>) + Send + Sync + 'static,
    {
        self.toggled.connect(slot)
    }
    /// Connects a callback for enabled-state changes.
    pub fn connect_enabled_changed<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(Arc<bool>) + Send + Sync + 'static,
    {
        self.enabled_changed.connect(slot)
    }
    /// Triggers the action when enabled and returns whether it fired.
    pub fn trigger(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        if self.checkable {
            self.set_checked(!self.checked);
        }
        self.triggered.emit();
        true
    }
}
/// Host container kinds that can expose actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionHostKind {
    /// Action is hosted by a menu.
    Menu,
    /// Action is hosted by a button-like widget.
    Button,
    /// Action is hosted by a toolbar.
    ToolBar,
}
/// Associates an action with a concrete UI host object.
#[derive(Debug, Clone)]
pub struct ActionBinding {
    /// Identifier of the bound action.
    pub action_id: String,
    /// Object id of the host widget.
    pub host_id: ObjectId,
    /// Host kind receiving the action.
    pub kind: ActionHostKind,
}
/// Canonicalizes a shortcut string so every spelling of *the same chord* maps to one key.
///
/// # Why aliases are folded, not just lower-cased
///
/// `Shortcut::from_string` maps `primary`, `cmd`, `command`, `cmdorctrl` and `ctrl` all to
/// [`Modifiers::PRIMARY`](crate::shortcut::Modifiers::PRIMARY) — that is what makes `"Cmd+Z"` and
/// `"Ctrl+Z"` parse to the *same* value. `Shortcut::to_string` (its canonical form) renders that
/// bit as `"Primary"`. So a bridge that only lower-cases would key `"ctrl+s"` and `"primary+s"`
/// as **two** entries for one chord, and a type-bound chord could never be triggered by the
/// `Ctrl` spelling a hand-written layout uses. Folding the aliases here is what makes both the
/// string API and the type API agree, wherever they are reached.
pub(crate) fn normalize_shortcut(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(|token| token.trim().to_lowercase())
        // Fold every spelling of the primary modifier onto `"primary"`, matching
        // `Shortcut::from_string`. Any other token is kept verbatim.
        //
        // `String::from` rather than `"primary".to_string()`: an allocation-frugal profile does
        // not pull in the `ToString` prelude, so the `to_string()` spelling failed to compile
        // under `--features mini` (E0599) — a defect the `mini` profile gate caught. The branch
        // must stay owned because the other arm holds the owned result of `to_lowercase()`.
        .map(|token| match token.as_str() {
            "cmd" | "command" | "cmdorctrl" | "ctrl" | "control" => String::from("primary"),
            _ => token,
        })
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>()
        .join("+")
}
