// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The refusal gate shared by every GUI probe (DEMO-007).
//!
//! Staging a probe means opening real windows or dialogs on the session it runs in, which is the
//! point of the probe but also why it must be a deliberate act: on a developer's live desktop each
//! run pops a window over their work. Each example used to carry its own byte-for-byte copy of
//! this check, so the wording and the environment variable could drift. The copy lives here; an
//! example gates the module declaration with the same `cfg` it used for its own function, so the
//! function exists exactly where it is called.

/// Whether the operator has allowed this probe to open a real window.
pub(crate) fn gui_probe_allowed() -> bool {
    if std::env::var("RW_GUI_PROBE_OK").ok().as_deref() == Some("1") {
        return true;
    }
    println!(
        "this probe opens real windows on the desktop it runs on; set RW_GUI_PROBE_OK=1 to stage \
         it (and prefer a machine whose screen nobody is using)"
    );
    false
}
