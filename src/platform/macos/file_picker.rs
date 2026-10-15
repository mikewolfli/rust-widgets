// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AppKit's own file picker, behind [`Platform::choose_path`](crate::platform::Platform::choose_path).
//!
//! # Why this is its own file
//!
//! The body is AppKit message sends, and a non-Apple host cannot compile this crate for
//! `x86_64-apple-darwin` (a C dependency in the graph needs an Apple toolchain). Keeping the body
//! free of crate-internal names — it depends on `objc`/`cocoa` and on the two types
//! `super::super::{PathPurpose, PathPickerError}` and nothing else — is what lets the same file be
//! compiled on its own for the target (see `docs/log/log-20261014-1.md`, probe P-2), so the code
//! is verified by a compiler rather than by review.
//!
//! # Which panel, and why one function covers all three purposes
//!
//! `NSSavePanel` is the base class and the one that owns the modal loop and the path answer;
//! `NSOpenPanel` is its subclass, which adds "can choose files / directories / many". So "open a
//! file" is an `NSOpenPanel` with files enabled, "choose a directory" is the same panel with
//! directories enabled and files off, and "save a file" is the base `NSSavePanel` — the three
//! purposes differ only in those flags.

use super::super::{PathPickerError, PathPurpose};
use cocoa::base::{id, nil, BOOL, NO};
// `NSAutoreleasePool` is the trait that provides `autorelease()`: without it the call below does
// not resolve, which is how the first probe compile of this file failed.
use cocoa::foundation::{NSAutoreleasePool, NSString};
use objc::{class, msg_send, sel, sel_impl};

/// `NSModalResponseOK`: what a panel's `runModal` returns when the user accepted.
///
/// Named rather than written as a bare `1` at the comparison, because it is the one number a
/// reader has to know to read the result handling below.
const NS_MODAL_RESPONSE_OK: i64 = 1;

/// Presents AppKit's file picker and answers with what the user chose.
///
/// # AppKit is main-thread-only
///
/// `-[NSSavePanel runModal]` runs a modal session, which only the main thread may do; from any
/// other thread AppKit raises an Objective-C exception, which Rust cannot catch. That case is
/// reported as [`PathPickerError::Unavailable`]: a caller can ask again from the loop's thread,
/// whereas a raised exception ends the process.
///
/// # What is not claimed
///
/// The returned path is not checked for existence or writability. A save panel legitimately names
/// a file that does not exist yet, and whether this process may open what the user picked is what
/// the caller's own `open`/`create` reports — with the error the OS actually produced.
pub(crate) fn choose_path(
    purpose: PathPurpose,
    title: &str,
) -> Result<Option<std::path::PathBuf>, PathPickerError> {
    // SAFETY: every message below is sent from the main thread (checked first, because a modal
    // session off-main raises) to objects AppKit owns for the duration of the call. The title
    // string is autoreleased and only read during `setTitle:`.
    unsafe {
        // The return type is annotated because `msg_send!` infers it from the comparison, and
        // `BOOL` is what the selector answers.
        let on_main_thread: BOOL = msg_send![class!(NSThread), isMainThread];
        if on_main_thread == NO {
            return Err(PathPickerError::Unavailable);
        }
        let panel: id = match purpose {
            // A save panel is the base class; asking for an open panel here would give the user a
            // chooser that cannot name a new file.
            PathPurpose::SaveFile => msg_send![class!(NSSavePanel), savePanel],
            _ => msg_send![class!(NSOpenPanel), openPanel],
        };
        if panel == nil {
            // AppKit answers nil only when it cannot allocate the panel. Reporting it as "no
            // picker on this backend" would send a host to its own fallback list for what is a
            // transient failure, so this is the other error.
            return Err(PathPickerError::Unavailable);
        }
        // An autoreleased `NSString` for the panel title. `init_str` returns +1, so it is handed
        // to the enclosing pool, which AppKit's main thread has.
        let title_object = NSString::alloc(nil).init_str(title).autorelease();
        let _: () = msg_send![panel, setTitle: title_object];

        match purpose {
            PathPurpose::SelectDirectory => {
                let _: () = msg_send![panel, setCanChooseFiles: NO];
                let _: () = msg_send![panel, setCanChooseDirectories: 1 as BOOL];
            }
            PathPurpose::OpenFile => {
                let _: () = msg_send![panel, setCanChooseFiles: 1 as BOOL];
                let _: () = msg_send![panel, setCanChooseDirectories: NO];
            }
            // A save panel chooses a file by definition; the two flags do not apply to it.
            PathPurpose::SaveFile => {}
        }
        // One path is what `choose_path` answers with, so a panel that allowed many would throw
        // away the rest silently.
        let _: () = msg_send![panel, setAllowsMultipleSelection: NO];

        // Annotated for the same reason as the main-thread check above: `runModal` answers
        // `NSModalResponse`, which is an `NSInteger`.
        let response: i64 = msg_send![panel, runModal];
        if response != NS_MODAL_RESPONSE_OK {
            // Cancel is a complete answer, not a failure: see `PathPickerError`.
            return Ok(None);
        }
        let url: id = msg_send![panel, URL];
        if url == nil {
            return Ok(None);
        }
        let path_object: id = msg_send![url, path];
        if path_object == nil {
            return Ok(None);
        }
        let utf8: *const std::os::raw::c_char = msg_send![path_object, UTF8String];
        if utf8.is_null() {
            return Ok(None);
        }
        // The bytes belong to `NSString`; the path is copied out of them here rather than borrowed,
        // so nothing outlives the panel.
        let text = std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned();
        Ok(Some(std::path::PathBuf::from(text)))
    }
}
