// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The structured diagnostic model (BLUE31 §2.8).
//!
//! Every finding the CLI can report has a stable `RWCLI-NNN` code, so a CI job selects on the
//! code rather than on English prose. The type is deliberately flat: it is written as one JSON
//! object per line in `--json` mode, and a nested shape would only make that harder to consume.

use serde_json::{json, Value};

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The command cannot produce a correct result.
    Error,
    /// The result is usable but a gap was recorded.
    Warning,
    /// Context a human needs to read the report; never affects the exit code.
    Note,
}

impl Severity {
    /// The stable spelling used in the text and JSON forms.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

/// One finding, located as precisely as the stage that produced it could locate it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable code (`RWCLI-001` …).
    pub code: &'static str,
    /// How serious the finding is.
    pub severity: Severity,
    /// The file the finding is about.
    pub file: String,
    /// One-based line, or `0` when the finding is not line-addressed.
    pub line: u32,
    /// One-based column, or `0` when the finding is not column-addressed.
    pub column: u32,
    /// The node the finding is about, when the stage knows it.
    pub node_path: Option<String>,
    /// What was found.
    pub message: String,
    /// How to fix it, when that is knowable.
    pub hint: Option<String>,
}

impl Diagnostic {
    /// Creates a file-level diagnostic (no line, column or node).
    pub fn file(code: &'static str, severity: Severity, file: &str, message: String) -> Self {
        Self {
            code,
            severity,
            file: String::from(file),
            line: 0,
            column: 0,
            node_path: None,
            message,
            hint: None,
        }
    }

    /// Creates a diagnostic located at a line/column.
    pub fn at(
        code: &'static str,
        severity: Severity,
        file: &str,
        line: u32,
        column: u32,
        message: String,
    ) -> Self {
        Self { line, column, ..Self::file(code, severity, file, message) }
    }

    /// Attaches the node the finding belongs to.
    pub fn with_node(mut self, node_path: String) -> Self {
        self.node_path = Some(node_path);
        self
    }

    /// Attaches a hint.
    pub fn with_hint(mut self, hint: String) -> Self {
        self.hint = Some(hint);
        self
    }

    /// The JSON form of one diagnostic.
    pub fn to_json(&self) -> Value {
        json!({
            "code": self.code,
            "severity": self.severity.as_str(),
            "file": self.file,
            "line": self.line,
            "column": self.column,
            "node_path": self.node_path,
            "message": self.message,
            "hint": self.hint,
        })
    }

    /// The human-readable form: `file:line:col: severity[CODE]: message`.
    pub fn to_text(&self) -> String {
        let mut location = self.file.clone();
        if self.line > 0 {
            location.push(':');
            location.push_str(&self.line.to_string());
            if self.column > 0 {
                location.push(':');
                location.push_str(&self.column.to_string());
            }
        }
        let mut text =
            format!("{location}: {}[{}]: {}", self.severity.as_str(), self.code, self.message);
        if let Some(node) = &self.node_path {
            text.push_str(&format!(" (node {node})"));
        }
        if let Some(hint) = &self.hint {
            text.push_str(&format!("\n  hint: {hint}"));
        }
        text
    }
}

/// RWCLI-001 — the document is not JSON.
pub const JSON_PARSE_FAILED: &str = "RWCLI-001";
/// RWCLI-002 — the document parses but does not match the project contract.
pub const DOCUMENT_CONTRACT: &str = "RWCLI-002";
/// RWCLI-006 — a style field with no property carrier (`StyleGap`).
pub const STYLE_GAP: &str = "RWCLI-006";
/// RWCLI-007 — a node the generator refused.
pub const GENERATOR_UNSUPPORTED: &str = "RWCLI-007";
/// RWCLI-008 — a container exceeded the target's capacity.
pub const CAPACITY_OVERFLOW: &str = "RWCLI-008";
/// RWCLI-011 — a committed artifact does not match the document.
pub const ARTIFACT_DRIFT: &str = "RWCLI-011";
/// RWCLI-012 — an I/O operation failed.
pub const IO_FAILED: &str = "RWCLI-012";
