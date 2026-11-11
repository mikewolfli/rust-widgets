// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Document-value forms of an action: the serializable record and the shared live state.
//!
//! Both live here rather than in `mod.rs` so the module root stays declarations and
//! re-exports (principle #8). Every item keeps its historical `action::` path through the
//! re-export in `src/action/mod.rs`.

use super::Action;
use crate::compat::{format, String};

/// A serializable command record: the fields a project document persists.
///
/// # Why this is separate from [`Action`]
///
/// [`Action`] owns live signals, so it is not a document value: it cannot be compared for
/// equality, and its shortcut binding lives in [`crate::action::ActionManager`] rather than on the action. A
/// `ActionDefinition` is the **data** half — id, label, flags, shortcut and menu role — so the
/// JSON round trip has one stable shape while the runtime type keeps its signals. The field
/// order below is the order the writer emits, deliberately, so two exports of one command are
/// byte-identical.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActionDefinition {
    /// Stable identifier a menu/toolbar node references.
    pub id: String,
    /// Visible label.
    pub text: String,
    /// Whether the command keeps a checked state.
    pub checkable: bool,
    /// Current checked state (meaningful only when `checkable`).
    pub checked: bool,
    /// Whether hosts should present the command.
    pub visible: bool,
    /// Whether the command can currently be triggered.
    pub enabled: bool,
    /// Canonical shortcut spelling, or empty when unbound.
    pub shortcut: String,
    /// Platform menu role token, or the empty string for an ordinary command.
    pub role: String,
}

impl ActionDefinition {
    /// The stable JSON spelling of this record.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"id\":\"{}\",\"text\":\"{}\",\"checkable\":{},\"checked\":{},\"visible\":{},\"enabled\":{},\"shortcut\":\"{}\",\"role\":\"{}\"}}",
            json_escape(&self.id),
            json_escape(&self.text),
            self.checkable,
            self.checked,
            self.visible,
            self.enabled,
            json_escape(&self.shortcut),
            json_escape(&self.role),
        )
    }

    /// Reads a record written by [`Self::to_json`].
    ///
    /// `id` and `text` are required; the flags default to the runtime defaults, so an older
    /// document that predates a flag still loads. An unknown key is ignored deliberately — a
    /// newer document must not make an older reader fail.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let id = json_string_field(text, "id")
            .ok_or_else(|| String::from("action JSON has no `id` string field"))?;
        let label = json_string_field(text, "text")
            .ok_or_else(|| String::from("action JSON has no `text` string field"))?;
        Ok(Self {
            id,
            text: label,
            checkable: json_bool_field(text, "checkable").unwrap_or(false),
            checked: json_bool_field(text, "checked").unwrap_or(false),
            // Absent in documents written before M-018; those commands were visible.
            visible: json_bool_field(text, "visible").unwrap_or(true),
            enabled: json_bool_field(text, "enabled").unwrap_or(true),
            shortcut: json_string_field(text, "shortcut").unwrap_or_default(),
            role: json_string_field(text, "role").unwrap_or_default(),
        })
    }

    /// Builds a live command from this record.
    pub fn to_action(&self) -> Action {
        let mut action = Action::new(self.id.clone(), self.text.clone());
        action.set_enabled(self.enabled);
        action.set_checkable(self.checkable);
        action.set_visible(self.visible);
        if self.checkable {
            action.set_checked(self.checked);
        }
        action
    }

    /// Captures the data half of a live command.
    pub fn from_action(action: &Action) -> Self {
        Self {
            id: action.id.clone(),
            text: action.text.clone(),
            checkable: action.is_checkable(),
            checked: action.is_checked(),
            visible: action.is_visible(),
            enabled: action.is_enabled(),
            shortcut: String::new(),
            role: String::new(),
        }
    }
}

/// The presentable state of one command, shared between every host showing it.
///
/// # Why a shared cell rather than a query in `draw`
///
/// A signal callback cannot borrow the host widget, so a menu row that wanted to repaint on
/// `action_toggled` had nowhere to write. The cell is that place: the callback writes here, and
/// the host applies the snapshot when it next refreshes. One cell per `(action, host)` pair, not
/// one global table, so a host can be dropped without leaving a subscriber pointing at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionLiveState {
    /// The label the command currently carries.
    pub text: String,
    /// Whether the command can be triggered.
    pub enabled: bool,
    /// Whether hosts should present the command.
    pub visible: bool,
    /// Whether a checkable command is checked.
    pub checkable: bool,
    /// The current checked state.
    pub checked: bool,
}

impl ActionLiveState {
    /// Captures the state a definition declares.
    pub fn from_definition(definition: &ActionDefinition) -> Self {
        Self {
            text: definition.text.clone(),
            enabled: definition.enabled,
            visible: definition.visible,
            checkable: definition.checkable,
            checked: definition.checked,
        }
    }
}

impl Action {
    /// Serializes this command's own state.
    ///
    /// The shortcut is empty here because a binding belongs to [`crate::action::ActionManager`]; use
    /// [`crate::action::ActionManager::action_definition`] to export a command together with its chord.
    pub fn to_json(&self) -> String {
        ActionDefinition::from_action(self).to_json()
    }

    /// Rebuilds a command from [`Self::to_json`].
    pub fn from_json(text: &str) -> Result<Self, String> {
        Ok(ActionDefinition::from_json(text)?.to_action())
    }
}

/// Escapes the characters a JSON string may not carry raw.
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
fn json_string_field(text: &str, field: &str) -> Option<String> {
    let value = json_raw_field(text, field)?;
    let rest = value.strip_prefix('"')?;
    let mut chars = rest.chars();
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

/// Reads one flat boolean field from a single-level JSON object.
fn json_bool_field(text: &str, field: &str) -> Option<bool> {
    let value = json_raw_field(text, field)?;
    if value.starts_with("true") {
        Some(true)
    } else if value.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// The raw text after `"field":`, trimmed of leading whitespace.
fn json_raw_field<'a>(text: &'a str, field: &str) -> Option<&'a str> {
    let key = format!("\"{}\"", field);
    let start = text.find(&key)? + key.len();
    let rest = &text[start..];
    let colon = rest.find(':')? + 1;
    Some(rest[colon..].trim_start())
}
