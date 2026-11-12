// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Win32 virtual-key normalisation at the Windows message boundary.

/// Translates a Win32 virtual-key code into the framework's key-code convention.
///
/// # Why only function keys need this
///
/// `wParam` of `WM_KEYDOWN` is a virtual-key code. For letters, digits, arrows, Enter, Escape,
/// Backspace, Delete, space and Tab the value already equals the framework's code, which is why
/// forwarding it unchanged is correct for those. `VK_F1`..`VK_F12` are the exception: Win32
/// reports them as `112..=123`, but the framework reserves that range for lowercase `p`..`z`
/// (`shortcut::Key::from_key_code` folds case, so `112` parses as `Key::P`). Forwarding the raw
/// value made every function-key shortcut silently match a letter.
///
/// The Linux (`linux::canvas::logical_key`) and macOS (`macos::accelerator::logical_key`)
/// backends already map their native function keys into `key_codes::F1..=F12`; this is the same
/// mapping at the Windows boundary, so a control sees one convention (principles #35/#101).
pub(crate) fn framework_key(vk: u32) -> u32 {
    const VK_F1: u32 = 0x70;
    const VK_F12: u32 = 0x7B;
    if (VK_F1..=VK_F12).contains(&vk) {
        crate::event::key_codes::F1 + (vk - VK_F1)
    } else {
        vk
    }
}
