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
    let mut primary = false;
    let mut alt = false;
    let mut shift = false;
    let mut meta = false;
    let mut key: String = String::new();

    for token in shortcut.split('+').map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let lower = token.to_lowercase();
        match lower.as_str() {
            // The string API folds every spelling of the primary modifier onto
            // `primary`, exactly as `Shortcut::from_string` does. `Ctrl` *is*
            // `Primary` for a hand-written layout: on macOS that is Command, on
            // Windows/Linux it is Control.
            "cmd" | "command" | "cmdorctrl" | "ctrl" | "control" | "primary" => primary = true,
            // `Option` is the macOS spelling of the Alt key; both must agree.
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            // Meta/Win/Super are one physical modifier, so fold them together.
            "meta" | "win" | "super" => meta = true,
            // The last non-modifier token is the key, matching `Shortcut::from_string`.
            _ => key = lower,
        }
    }

    // Emit modifiers in the same canonical order `Shortcut::format_shortcut` uses,
    // so `"Ctrl+Shift+S"` and `"Shift+Ctrl+S"` both normalize to `primary+shift+s`.
    let mut parts: Vec<String> = Vec::new();
    if primary {
        parts.push(String::from("primary"));
    }
    if alt {
        parts.push(String::from("alt"));
    }
    if shift {
        parts.push(String::from("shift"));
    }
    if meta {
        parts.push(String::from("meta"));
    }
    if !key.is_empty() {
        parts.push(key);
    }
    parts.join("+")
}

/// Canonicalizes a typed [`Shortcut`](crate::shortcut::Shortcut) for the action
/// registry's reverse-lookup map.
///
/// Unlike [`normalize_shortcut`], this keeps
/// [`Modifiers::CTRL`](crate::shortcut::Modifiers::CTRL) distinct from
/// [`Modifiers::PRIMARY`](crate::shortcut::Modifiers::PRIMARY), because
/// `Shortcut::ctrl(S)` and `Shortcut::primary(S)` are different chords
/// (`ShortcutManager` registers both). The string API ([`normalize_shortcut`])
/// keeps folding `Ctrl` onto `primary`; that convention is deliberately isolated
/// from this typed bridge.
pub(crate) fn canonicalize_shortcut_type(shortcut: &crate::shortcut::Shortcut) -> String {
    shortcut.format_shortcut().to_lowercase()
}
