// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! EX4-02: the env-gated designer autosave follows apply, undo and redo.
//!
//! This lives in its own integration-test binary because the setting is a process-global
//! environment variable: the library's unit tests run in parallel threads, so a `set_var` there
//! would be observed by every concurrently running `apply` and the assertions would race. A
//! separate binary gets a process of its own, which is also how a real designer shell sets it.
//!
//! # Why the file is gated as a whole
//!
//! `rust_widgets::designer` is `cfg(designer_tooling)` — `full_widgets` **and** the `designer`
//! feature, because a `mini`/`embedded`/`tablet`/`mobile` build is the *target* of a generation, not
//! its host (BLUE19 §5.3.4). This file had no inner gate, so `cargo test --features embedded` (a CI
//! profile) did not skip it: the binary failed to compile with `could not find designer in
//! rust_widgets`. Every sibling probe in this directory opens with the same `#![cfg(...)]` line, and
//! this is the one that was missing it.
#![cfg(designer_tooling)]

use std::path::PathBuf;

use rust_widgets::designer::{DesignerSession, AUTOSAVE_ENV_VAR};

/// Restores the environment and removes the autosave file even when an assertion fails.
struct AutosaveGuard {
    path: PathBuf,
    previous: Option<String>,
}

impl AutosaveGuard {
    fn new() -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("rust-widgets-designer-autosave-{}.json", std::process::id()));
        let previous = std::env::var(AUTOSAVE_ENV_VAR).ok();
        std::env::set_var(AUTOSAVE_ENV_VAR, &path);
        Self { path, previous }
    }
}

impl Drop for AutosaveGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var(AUTOSAVE_ENV_VAR, value),
            None => std::env::remove_var(AUTOSAVE_ENV_VAR),
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

fn saved(guard: &AutosaveGuard) -> String {
    std::fs::read_to_string(&guard.path)
        .unwrap_or_else(|error| panic!("the autosave file must exist at {:?}: {error}", guard.path))
}

#[test]
fn the_configured_autosave_file_follows_the_document_and_the_gate() {
    let guard = AutosaveGuard::new();
    let mut session = DesignerSession::open("before");

    session.apply("after", "Set text");
    assert_eq!(saved(&guard), "after");

    session.undo().expect("the edit is undoable");
    assert_eq!(saved(&guard), "before");

    session.redo().expect("the edit is redoable");
    assert_eq!(saved(&guard), "after");

    // The gate's other half: with the variable removed the same edit performs no write.
    std::env::remove_var(AUTOSAVE_ENV_VAR);
    let mut unaudited = DesignerSession::open("off-before");
    unaudited.apply("off-after", "Set text");
    assert!(
        !unaudited.save_if_configured().expect("an unset variable is not an error"),
        "an unset variable must mean autosave is off"
    );
}
