// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AppKit environment facts for the cocoa (legacy) backend.
//!
//! BLUE28 B28.06: the style layer already had `high_contrast`, `motion_preference` and `locale`,
//! but the platform side forwarded none of them. This module is the AppKit half of the answer:
//! `NSWorkspace`'s accessibility display flags and `NSLocale.currentLocale`. The values are read
//! on demand (the style layer takes a snapshot once per frame), never cached here — macOS posts
//! notifications when these change, and a caller that polls sees the new value.
//!
//! `NSWorkspace` and `NSLocale` are main-thread-only in practice; every entry point answers `None`
//! off the main thread rather than messaging AppKit from a worker and raising.

use crate::compat::String;
use crate::compat::Vec;
use cocoa::base::{id, nil, BOOL, NO};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::{c_char, CStr};

/// Whether `+[NSWorkspace sharedWorkspace]` implements `selector`.
///
/// # Safety
///
/// Called only from the AppKit main thread, and `selector` is one of the two literal selectors
/// below. The workspace is an AppKit singleton.
unsafe fn workspace_implements(selector: objc::runtime::Sel) -> Option<id> {
    let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
    if workspace == nil {
        return None;
    }
    let responds: BOOL = msg_send![workspace, respondsToSelector: selector];
    (responds != NO).then_some(workspace)
}

/// Whether the user asked for increased contrast.
///
/// `NSWorkspace.accessibilityDisplayShouldIncreaseContrast` is the same source winit's AppKit
/// backend reads. `None` off the main thread or on an AppKit that does not implement the selector.
pub(crate) fn high_contrast() -> Option<bool> {
    if !super::types::is_main_thread() {
        return None;
    }
    // SAFETY: main thread established; the workspace and the selector are immutable AppKit facts.
    unsafe {
        let workspace = workspace_implements(sel!(accessibilityDisplayShouldIncreaseContrast))?;
        let answer: BOOL = msg_send![workspace, accessibilityDisplayShouldIncreaseContrast];
        Some(answer != NO)
    }
}

/// Whether the user asked for reduced motion.
///
/// The macOS "Reduce motion" accessibility setting. `None` off the main thread or when the
/// selector is unavailable.
pub(crate) fn reduce_motion() -> Option<bool> {
    if !super::types::is_main_thread() {
        return None;
    }
    // SAFETY: as above.
    unsafe {
        let workspace = workspace_implements(sel!(accessibilityDisplayShouldReduceMotion))?;
        let answer: BOOL = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        Some(answer != NO)
    }
}

/// The installed families, their style names and backing files, through CoreText.
///
/// `NSFontManager.availableFontFamilies` reports names only; CoreText's `kCTFontURLAttribute` is
/// what supplies the file the renderer can register. The enumeration is not main-thread-only.
pub(crate) fn system_fonts() -> Vec<crate::platform::types::SystemFont> {
    crate::platform::fonts_apple::system_fonts()
}

/// The user's locale, normalized by `NSLocale.localeIdentifier` (for example `en_US`).
///
/// AppKit returns an autoreleased `NSString`; the UTF-8 bytes are copied before the call returns,
/// so no object outlives this function.
pub(crate) fn system_locale() -> Option<&'static str> {
    // SAFETY: every object messaged is an AppKit singleton or a child of one, and the UTF-8 buffer
    // is read before the autorelease pool that owns it can drain.
    unsafe {
        let locale: id = msg_send![class!(NSLocale), currentLocale];
        if locale == nil {
            return None;
        }
        let identifier: id = msg_send![locale, localeIdentifier];
        if identifier == nil {
            return None;
        }
        let utf8: *const c_char = msg_send![identifier, UTF8String];
        if utf8.is_null() {
            return None;
        }
        let tag: String = CStr::from_ptr(utf8).to_string_lossy().into_owned();
        crate::platform::locale::intern(Some(tag.replace('_', "-"))).or_else(|| {
            crate::platform::locale::intern(crate::platform::locale::from_environment())
        })
    }
}
