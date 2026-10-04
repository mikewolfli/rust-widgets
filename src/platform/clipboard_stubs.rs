// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Platform-specific rich clipboard stubs.
//! These will be replaced with real platform clipboard bindings.

/// The CF_HTML wire format used by the Windows clipboard's `HTML Format`.
///
/// Kept as a platform-independent module (gated to compile on Windows and under
/// `cfg(test)`) so the byte-offset codec can be unit-tested on a macOS host, where
/// the `windows` backend below never compiles. The codec itself touches no Win32 API.
#[cfg(any(target_os = "windows", test))]
pub(crate) mod cf_html {
    use crate::compat::{format, String, ToString, Vec};

    /// The literal HTML envelope CF_HTML wraps a fragment in.
    const PREFIX: &str = "<html><body>";
    const START_MARKER: &str = "<!--StartFragment-->";
    const END_MARKER: &str = "<!--EndFragment-->";
    const SUFFIX: &str = "</body></html>";

    /// Formats an HTML fragment into the Windows CF_HTML clipboard document.
    ///
    /// `StartFragment`/`EndFragment` are **byte** offsets from the start of the whole
    /// document into the fragment. They must therefore include the `<html><body>` prefix
    /// plus the start marker, not just the marker — counting only the marker made both
    /// offsets 12 bytes early, so a paste carried the marker's tail and truncated the
    /// body.
    pub(crate) fn format_cf_html(html: &str) -> Vec<u8> {
        let full_html = format!("{PREFIX}{START_MARKER}{html}{END_MARKER}{SUFFIX}");

        let start_fragment_offset = PREFIX.len() + START_MARKER.len();
        let end_fragment_offset = start_fragment_offset + html.len();

        // Build the header with a fixed-width placeholder, then substitute the real
        // offsets. Every offset is zero-padded to the same width, so the header length
        // is identical in both passes and the byte arithmetic stays exact.
        let placeholder = "0000000000";
        let header_template = format!(
            "Version:0.9\r\nStartHTML:{placeholder}\r\nEndHTML:{placeholder}\r\nStartFragment:{placeholder}\r\nEndFragment:{placeholder}\r\n"
        );

        let start_html = header_template.len();
        let end_html = start_html + full_html.len();
        let start_fragment = start_html + start_fragment_offset;
        let end_fragment = start_html + end_fragment_offset;

        format!(
            "Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\nStartFragment:{start_fragment:010}\r\nEndFragment:{end_fragment:010}\r\n{full_html}"
        )
        .into_bytes()
    }

    /// Extracts the HTML fragment from a CF_HTML document.
    ///
    /// Prefers the header's `StartFragment`/`EndFragment` offsets; when they are absent or
    /// invalid, falls back to scanning for the markers, and finally to the whole document.
    pub(crate) fn parse_cf_html(cf_html: &str) -> String {
        let mut start_fragment = 0usize;
        let mut end_fragment = 0usize;

        for line in cf_html.lines() {
            if let Some(val) = line.strip_prefix("StartFragment:") {
                start_fragment = val.trim().parse().unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("EndFragment:") {
                end_fragment = val.trim().parse().unwrap_or(0);
            }
        }

        if start_fragment > 0 && end_fragment > start_fragment && end_fragment <= cf_html.len() {
            cf_html[start_fragment..end_fragment].to_string()
        } else {
            let start_tag = "<!--StartFragment-->";
            let end_tag = "<!--EndFragment-->";

            if let Some(start) = cf_html.find(start_tag) {
                let content_start = start + start_tag.len();
                if let Some(end) = cf_html[content_start..].find(end_tag) {
                    return cf_html[content_start..content_start + end].to_string();
                }
            }
            cf_html.to_string()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn ascii_fragment_round_trips_without_marker_or_truncation() {
            let encoded = format_cf_html("<b>bold</b>");
            let doc = String::from_utf8(encoded).unwrap();
            assert_eq!(parse_cf_html(&doc), "<b>bold</b>");
            // The header's offsets must point exactly at the fragment, not 12 bytes early.
            let start = doc.find("<b>bold</b>").unwrap();
            for line in doc.lines() {
                if let Some(val) = line.strip_prefix("StartFragment:") {
                    assert_eq!(val.trim().parse::<usize>().unwrap(), start);
                }
            }
        }

        #[test]
        fn chinese_fragment_round_trips_with_byte_offsets() {
            let encoded = format_cf_html("中文内容");
            let doc = String::from_utf8(encoded).unwrap();
            assert_eq!(parse_cf_html(&doc), "中文内容");
        }

        #[test]
        fn empty_fragment_round_trips_as_empty() {
            let encoded = format_cf_html("");
            let doc = String::from_utf8(encoded).unwrap();
            assert_eq!(parse_cf_html(&doc), "");
        }
    }
}

#[cfg(all(target_os = "macos", feature = "cocoa-legacy"))]
pub mod macos {
    //! Real macOS clipboard using NSPasteboard rich content APIs.
    //! Reference: NSPasteboard, NSPasteboardItem, NSPasteboardItemDataProvider

    use super::super::clipboard::{ClipboardContent, RichClipboardBackend};
    // `compat` rather than the prelude: `#![no_std]` removes the prelude on the
    // `mini` profile, and this module is reachable from `mini,cocoa-legacy`.
    // Only `String` is needed; `CStr::to_string_lossy` resolves inherently.
    use crate::compat::String;
    use cocoa::base::{id, nil, BOOL, YES};
    use cocoa::foundation::{NSAutoreleasePool, NSString};
    use objc::{class, msg_send, sel, sel_impl};

    /// An autoreleased `NSString` for a Rust `&str`.
    ///
    /// `NSString::init_str` returns a +1-retained object, so every pasteboard type string and every
    /// value written to the pasteboard used to leak one. `autorelease` hands ownership to the
    /// enclosing pool, so the string is reclaimed after the current run-loop turn — the correct
    /// lifetime for a value that is only read during the following `NSPasteboard` call.
    ///
    /// # SAFETY
    /// Must be called on a thread with an active autorelease pool (AppKit's main thread has one).
    unsafe fn nsstring(s: &str) -> id {
        NSString::alloc(nil).init_str(s).autorelease()
    }

    /// macOS rich clipboard backed by the general `NSPasteboard`.
    pub struct MacOsClipboard;

    impl MacOsClipboard {
        /// Get the general pasteboard and clear its contents.
        unsafe fn prepare_pasteboard() -> id {
            let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
            let _: i64 = msg_send![pb, clearContents];
            pb
        }

        /// Read plain text from NSPasteboard.
        unsafe fn read_plain_text(pb: id) -> Option<String> {
            let items: id = msg_send![pb, pasteboardItems];
            let count: usize = msg_send![items, count];
            if count == 0 {
                return None;
            }
            let item: id = msg_send![items, objectAtIndex: 0u64];
            let str_id: id = msg_send![item, stringForType: nsstring("public.utf8-plain-text")];
            if str_id == nil {
                return None;
            }
            let c_str: *const std::os::raw::c_char = msg_send![str_id, UTF8String];
            if c_str.is_null() {
                return None;
            }
            Some(std::ffi::CStr::from_ptr(c_str).to_string_lossy().into_owned())
        }
    }

    impl RichClipboardBackend for MacOsClipboard {
        fn set_contents(&self, content: ClipboardContent) -> bool {
            let result = std::panic::catch_unwind(|| unsafe {
                let pb = Self::prepare_pasteboard();
                match &content {
                    ClipboardContent::Text(text) => {
                        let item: id = msg_send![class!(NSPasteboardItem), alloc];
                        let item: id = msg_send![item, init];
                        let ns_string = nsstring(text);
                        let success: BOOL = msg_send![item, setString: ns_string forType: nsstring("public.utf8-plain-text")];
                        if success == YES {
                            let arr: id = msg_send![class!(NSArray), arrayWithObject: item];
                            // The **writeObjects** result is the actual system write. A successful
                            // `setString:forType:` only built the item; it says nothing about whether
                            // the pasteboard accepted it. Returning the write result is what makes a
                            // failed system write report false rather than a success that never
                            // reached the clipboard.
                            let written: BOOL = msg_send![pb, writeObjects: arr];
                            written == YES
                        } else {
                            false
                        }
                    }
                    ClipboardContent::Html { html, plain } => {
                        let item: id = msg_send![class!(NSPasteboardItem), alloc];
                        let item: id = msg_send![item, init];

                        let ns_html = nsstring(html);
                        let html_ok: BOOL =
                            msg_send![item, setString: ns_html forType: nsstring("public.html")];

                        let ns_plain = nsstring(plain);
                        let plain_ok: BOOL = msg_send![item, setString: ns_plain forType: nsstring("public.utf8-plain-text")];

                        if html_ok == YES || plain_ok == YES {
                            let arr: id = msg_send![class!(NSArray), arrayWithObject: item];
                            // See the `Text` arm: the system write is the source of truth for success.
                            let written: BOOL = msg_send![pb, writeObjects: arr];
                            written == YES
                        } else {
                            false
                        }
                    }
                    _ => {
                        log::warn!("[macOS clipboard] non-text/html format not yet supported");
                        false
                    }
                }
            });
            result.unwrap_or(false)
        }

        fn get_contents(&self) -> Option<ClipboardContent> {
            let result = std::panic::catch_unwind(|| unsafe {
                let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
                let items: id = msg_send![pb, pasteboardItems];
                let count: usize = msg_send![items, count];
                if count == 0 {
                    return None;
                }
                let item: id = msg_send![items, objectAtIndex: 0u64];

                // Try HTML first
                let html_id: id = msg_send![item, stringForType: nsstring("public.html")];
                if html_id != nil {
                    let c_str: *const std::os::raw::c_char = msg_send![html_id, UTF8String];
                    if !c_str.is_null() {
                        let html = std::ffi::CStr::from_ptr(c_str).to_string_lossy().into_owned();
                        let plain = Self::read_plain_text(pb).unwrap_or_default();
                        return Some(ClipboardContent::Html { html, plain });
                    }
                }

                // Fall back to plain text
                Self::read_plain_text(pb).map(ClipboardContent::Text)
            });
            result.unwrap_or(None)
        }

        fn has_format(&self, content_type: &str) -> bool {
            let result = std::panic::catch_unwind(|| unsafe {
                let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
                let ns_type = nsstring(content_type);
                let arr: id = msg_send![class!(NSArray), arrayWithObject: ns_type];
                let available: id = msg_send![pb, availableTypeFromArray: arr];
                available != nil
            });
            result.unwrap_or(false)
        }
    }
}

#[cfg(target_os = "windows")]
pub mod windows {
    //! Real Windows clipboard using Win32 clipboard API.
    //! Reference: OpenClipboard, SetClipboardData, GetClipboardData, CF_TEXT

    use super::super::clipboard::{ClipboardContent, RichClipboardBackend};
    // Alloc types come from the compat bridge: `mini` is `no_std`, so the std
    // prelude that normally supplies `String`/`Vec`/`to_string` is suppressed
    // and this module failed to compile with 8 errors under
    // `--target x86_64-pc-windows-msvc --features mini`.
    use crate::compat::{String, ToString, Vec};
    use winapi::shared::minwindef::{FALSE, UINT};
    use winapi::um::winbase::GlobalAlloc;
    use winapi::um::winbase::{GlobalLock, GlobalSize, GlobalUnlock, GHND};
    use winapi::um::winuser::CF_UNICODETEXT;
    use winapi::um::winuser::{
        CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, RegisterClipboardFormatA,
        SetClipboardData,
    };

    /// Pure-text clipboard backend for Windows, backed by the real Win32
    /// clipboard API (`OpenClipboard`/`SetClipboardData`).
    ///
    /// Only compiled on Windows targets: the `use` block above imports the Win32
    /// symbols unconditionally, so this type cannot exist elsewhere.
    pub struct WindowsClipboard;

    impl WindowsClipboard {
        unsafe fn read_unicode_text() -> Option<String> {
            let h_mem = GetClipboardData(CF_UNICODETEXT);
            if h_mem.is_null() {
                return None;
            }
            let ptr = GlobalLock(h_mem);
            if ptr.is_null() {
                return None;
            }
            let byte_size = GlobalSize(h_mem);
            let char_count = byte_size / 2;
            let wide_slice = std::slice::from_raw_parts(ptr as *const u16, char_count as usize);
            let nul_pos = wide_slice.iter().position(|&c| c == 0).unwrap_or(0);
            let result = String::from_utf16_lossy(&wide_slice[..nul_pos]);
            GlobalUnlock(h_mem);
            Some(result)
        }

        /// Format HTML content into the Windows CF_HTML clipboard format.
        ///
        /// Delegates to the platform-independent [`super::cf_html`] codec so the byte
        /// offsets stay testable on a non-Windows host.
        fn format_cf_html(html: &str) -> Vec<u8> {
            super::cf_html::format_cf_html(html)
        }

        /// Parse a CF_HTML formatted string and extract the HTML fragment.
        fn parse_cf_html(cf_html: &str) -> String {
            super::cf_html::parse_cf_html(cf_html)
        }

        /// Get the registered clipboard format ID for "HTML Format".
        unsafe fn html_format_id() -> UINT {
            let name = std::ffi::CString::new("HTML Format").unwrap();
            // winapi's *-A entry points take `*const i8` (CHAR); `c_char` is
            // `i8` on Windows MSVC, matching the C ABI.
            RegisterClipboardFormatA(name.as_ptr() as *const std::os::raw::c_char)
        }
    }

    impl RichClipboardBackend for WindowsClipboard {
        fn set_contents(&self, content: ClipboardContent) -> bool {
            let result = std::panic::catch_unwind(|| unsafe {
                if OpenClipboard(std::ptr::null_mut()) == FALSE {
                    return false;
                }

                // Win32 requires the clipboard to be emptied between `OpenClipboard`
                // and `SetClipboardData`. Without this the new data does not replace
                // what the previous owner left behind, so the result depends on the
                // clipboard's prior contents — and a stale `CF_HTML` entry keeps
                // winning over the text we are about to publish.
                EmptyClipboard();

                let mut success = true;

                match &content {
                    ClipboardContent::Text(text) => {
                        let wide: Vec<u16> =
                            text.encode_utf16().chain(std::iter::once(0)).collect();
                        let bytes = wide.len() * 2;
                        let h_mem = GlobalAlloc(GHND, bytes);
                        if h_mem.is_null() {
                            CloseClipboard();
                            return false;
                        }
                        let ptr = GlobalLock(h_mem);
                        if ptr.is_null() {
                            GlobalUnlock(h_mem);
                            CloseClipboard();
                            return false;
                        }
                        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
                        GlobalUnlock(h_mem);
                        let result = SetClipboardData(CF_UNICODETEXT, h_mem);
                        CloseClipboard();
                        !result.is_null()
                    }
                    ClipboardContent::Html { html, plain } => {
                        // Set CF_HTML (HTML Format)
                        let cf_html_bytes = Self::format_cf_html(html);
                        let cf_html_len = cf_html_bytes.len();
                        let h_html = GlobalAlloc(GHND, cf_html_len);
                        if h_html.is_null() {
                            CloseClipboard();
                            return false;
                        }
                        let html_ptr = GlobalLock(h_html);
                        if html_ptr.is_null() {
                            GlobalUnlock(h_html);
                            CloseClipboard();
                            return false;
                        }
                        std::ptr::copy_nonoverlapping(
                            cf_html_bytes.as_ptr(),
                            html_ptr as *mut u8,
                            cf_html_len,
                        );
                        GlobalUnlock(h_html);

                        let cf_html_format = Self::html_format_id();
                        let html_set = SetClipboardData(cf_html_format, h_html);
                        if html_set.is_null() {
                            success = false;
                        }

                        // Set CF_UNICODETEXT (plain text fallback)
                        let wide: Vec<u16> =
                            plain.encode_utf16().chain(std::iter::once(0)).collect();
                        let bytes = wide.len() * 2;
                        let h_text = GlobalAlloc(GHND, bytes);
                        if h_text.is_null() {
                            CloseClipboard();
                            return false;
                        }
                        let text_ptr = GlobalLock(h_text);
                        if text_ptr.is_null() {
                            GlobalUnlock(h_text);
                            CloseClipboard();
                            return false;
                        }
                        std::ptr::copy_nonoverlapping(
                            wide.as_ptr(),
                            text_ptr as *mut u16,
                            wide.len(),
                        );
                        GlobalUnlock(h_text);
                        let text_set = SetClipboardData(CF_UNICODETEXT, h_text);
                        if text_set.is_null() {
                            success = false;
                        }

                        CloseClipboard();
                        success
                    }
                    _ => {
                        CloseClipboard();
                        log::warn!("[Windows clipboard] non-text/html format not yet supported");
                        false
                    }
                }
            });
            result.unwrap_or(false)
        }

        fn get_contents(&self) -> Option<ClipboardContent> {
            let result = std::panic::catch_unwind(|| unsafe {
                if OpenClipboard(std::ptr::null_mut()) == FALSE {
                    return None;
                }

                // Try HTML Format first
                let cf_html_format = Self::html_format_id();
                let h_html = GetClipboardData(cf_html_format);
                if !h_html.is_null() {
                    let ptr = GlobalLock(h_html);
                    if !ptr.is_null() {
                        let byte_size = GlobalSize(h_html);
                        let slice =
                            std::slice::from_raw_parts(ptr as *const u8, byte_size as usize);
                        // Find NUL terminator
                        let nul_pos = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
                        let cf_html_str = String::from_utf8_lossy(&slice[..nul_pos]).into_owned();

                        // Parse CF_HTML to extract the fragment
                        let html_content = Self::parse_cf_html(&cf_html_str);
                        let plain = Self::read_unicode_text().unwrap_or_default();

                        GlobalUnlock(h_html);
                        CloseClipboard();
                        return Some(ClipboardContent::Html { html: html_content, plain });
                    }
                }

                // Fall back to plain text
                let text = Self::read_unicode_text();
                CloseClipboard();
                text.map(ClipboardContent::Text)
            });
            result.unwrap_or(None)
        }

        fn has_format(&self, content_type: &str) -> bool {
            let result = std::panic::catch_unwind(|| unsafe {
                if OpenClipboard(std::ptr::null_mut()) == FALSE {
                    return false;
                }
                let has =
                    if content_type == "text/plain" || content_type == "public.utf8-plain-text" {
                        let h = GetClipboardData(CF_UNICODETEXT);
                        !h.is_null()
                    } else if content_type == "text/html" {
                        let cf_html_format = Self::html_format_id();
                        let h = GetClipboardData(cf_html_format);
                        !h.is_null()
                    } else {
                        false
                    };
                CloseClipboard();
                has
            });
            result.unwrap_or(false)
        }
    }
}

// ── macOS objc2 clipboard (feature = "macos") ──

/// macOS clipboard backend using objc2 (NSPasteboard via objc2-app-kit).
#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod objc2_macos {
    // macOS clipboard using objc2 NSPasteboard APIs.
    // Uses objc2 runtime messaging with NSPasteboard, NSPasteboardItem, and NSArray.

    use super::super::clipboard::{ClipboardContent, RichClipboardBackend};
    // `read_plain_text` returns `compat::String`, which under `mini` is
    // `alloc::string::String` — not the prelude's, because `#![no_std]` removes
    // that prelude. `CStr::to_string_lossy` resolves inherently, so only the
    // `String` name needs importing.
    use crate::compat::String;
    use objc2::class;
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSString;

    /// macOS clipboard backend using objc2 NSPasteboard bindings.
    pub struct MacOsObjc2Clipboard;

    impl MacOsObjc2Clipboard {
        /// Get the general pasteboard and clear its contents.
        #[allow(clippy::missing_safety_doc)]
        unsafe fn prepare_pasteboard() -> *mut AnyObject {
            let pb: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
            let _: i64 = msg_send![pb, clearContents];
            pb
        }

        /// Read plain text from NSPasteboard.
        #[allow(clippy::missing_safety_doc)]
        unsafe fn read_plain_text(pb: *mut AnyObject) -> Option<String> {
            let items: *mut AnyObject = msg_send![pb, pasteboardItems];
            let count: usize = msg_send![items, count];
            if count == 0 {
                return None;
            }
            let item: *mut AnyObject = msg_send![items, objectAtIndex: 0u64];
            let type_str = NSString::from_str("public.utf8-plain-text");
            let str_id: *mut AnyObject = msg_send![item, stringForType: &*type_str];
            if str_id.is_null() {
                return None;
            }
            let c_str: *const std::os::raw::c_char = msg_send![str_id, UTF8String];
            if c_str.is_null() {
                return None;
            }
            Some(std::ffi::CStr::from_ptr(c_str).to_string_lossy().into_owned())
        }
    }

    impl RichClipboardBackend for MacOsObjc2Clipboard {
        fn set_contents(&self, content: ClipboardContent) -> bool {
            let result = std::panic::catch_unwind(|| unsafe {
                let pb = Self::prepare_pasteboard();

                match &content {
                    ClipboardContent::Text(text) => {
                        let item: *mut AnyObject = msg_send![class!(NSPasteboardItem), alloc];
                        let item: *mut AnyObject = msg_send![item, init];
                        let ns_string = NSString::from_str(text);
                        let ns_type = NSString::from_str("public.utf8-plain-text");
                        let success: bool =
                            msg_send![item, setString: &*ns_string, forType: &*ns_type];
                        if success {
                            // The item must be written, not the bare string: `writeObjects:` takes an
                            // array of `NSPasteboardItem`s, and passing a string wrote nothing. The
                            // return value is the actual system write, which is henceforth the success
                            // answer — a built item is not a delivered one.
                            let arr: *mut AnyObject =
                                msg_send![class!(NSArray), arrayWithObject: item];
                            let written: bool = msg_send![pb, writeObjects: arr];
                            written
                        } else {
                            false
                        }
                    }
                    ClipboardContent::Html { html, plain } => {
                        let item: *mut AnyObject = msg_send![class!(NSPasteboardItem), alloc];
                        let item: *mut AnyObject = msg_send![item, init];

                        let ns_html = NSString::from_str(html);
                        let ns_html_type = NSString::from_str("public.html");
                        let html_ok: bool =
                            msg_send![item, setString: &*ns_html, forType: &*ns_html_type];

                        let ns_plain = NSString::from_str(plain);
                        let ns_plain_type = NSString::from_str("public.utf8-plain-text");
                        let plain_ok: bool =
                            msg_send![item, setString: &*ns_plain, forType: &*ns_plain_type];

                        if html_ok || plain_ok {
                            // See the `Text` arm: write the prepared `item` (not the raw strings) and
                            // report the system write's own result.
                            let arr: *mut AnyObject =
                                msg_send![class!(NSArray), arrayWithObject: item];
                            let written: bool = msg_send![pb, writeObjects: arr];
                            written
                        } else {
                            false
                        }
                    }
                    _ => {
                        log::warn!(
                            "[macOS objc2 clipboard] non-text/html format not yet supported"
                        );
                        false
                    }
                }
            });
            result.unwrap_or(false)
        }

        fn get_contents(&self) -> Option<ClipboardContent> {
            let result = std::panic::catch_unwind(|| unsafe {
                let pb: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
                let items: *mut AnyObject = msg_send![pb, pasteboardItems];
                let count: usize = msg_send![items, count];
                if count == 0 {
                    return None;
                }
                let item: *mut AnyObject = msg_send![items, objectAtIndex: 0u64];

                // Try HTML first
                let html_type = NSString::from_str("public.html");
                let html_id: *mut AnyObject = msg_send![item, stringForType: &*html_type];
                if !html_id.is_null() {
                    let c_str: *const std::os::raw::c_char = msg_send![html_id, UTF8String];
                    if !c_str.is_null() {
                        let html = std::ffi::CStr::from_ptr(c_str).to_string_lossy().into_owned();
                        let plain = Self::read_plain_text(pb).unwrap_or_default();
                        return Some(ClipboardContent::Html { html, plain });
                    }
                }

                // Fall back to plain text
                Self::read_plain_text(pb).map(ClipboardContent::Text)
            });
            result.unwrap_or(None)
        }

        fn has_format(&self, content_type: &str) -> bool {
            let result = std::panic::catch_unwind(|| unsafe {
                let pb: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
                let ns_type = NSString::from_str(content_type);
                let arr: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: &*ns_type];
                let available: *mut AnyObject = msg_send![pb, availableTypeFromArray: arr];
                !available.is_null()
            });
            result.unwrap_or(false)
        }
    }
}

// ── Linux clipboard (session-local store) ──

/// Linux clipboard backend (session-local store).
///
/// # Why `set_contents` returns `false`
///
/// Native Linux clipboard access needs a live desktop session (GTK / Wayland data-device / X11
/// selections), which this build tree does not link. The store below is genuinely useful **within
/// one process** — it round-trips text, HTML and images — but it never reaches the system clipboard,
/// so claiming success would be exactly the "reported success for something that did not happen"
/// failure rules #41/#53 forbid. `set_contents` therefore stores the value (so a same-process
/// `get_contents` works) and returns `false`, and no `Platform` backend returns this type from
/// `clipboard_backend()`. A real GTK/Wayland path can replace it without changing the contract.
#[cfg(target_os = "linux")]
pub mod linux {

    use super::super::clipboard::{ClipboardContent, RichClipboardBackend};
    use crate::compat::{lock, Mutex};

    /// Session-local clipboard store for Linux.
    #[derive(Debug, Default)]
    pub struct LinuxClipboard {
        content: Mutex<Option<ClipboardContent>>,
    }

    impl LinuxClipboard {
        /// Create a new empty Linux clipboard backend.
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl RichClipboardBackend for LinuxClipboard {
        fn set_contents(&self, content: ClipboardContent) -> bool {
            // Stored for same-process reads, but this is not the system clipboard.
            *lock(&self.content) = Some(content);
            false
        }

        fn get_contents(&self) -> Option<ClipboardContent> {
            lock(&self.content).clone()
        }

        fn has_format(&self, content_type: &str) -> bool {
            lock(&self.content).as_ref().is_some_and(|c| c.content_type() == content_type)
        }
    }
}

// ── WASM clipboard (session-local store) ──

/// WASM/WebAssembly clipboard backend (session-local store).
///
/// The browser `navigator.clipboard` API is entirely Promise-based, so it cannot be driven from the
/// synchronous trait methods without blocking the event loop. This store is fully functional within
/// a single WASM session but never touches the browser clipboard, so — like the Linux store above —
/// `set_contents` returns `false` rather than claiming a system-clipboard write that did not happen.
#[cfg(feature = "wasm")]
pub mod wasm {
    use super::super::clipboard::{ClipboardContent, RichClipboardBackend};
    use crate::compat::{lock, Mutex};

    /// Session-local clipboard store for WASM.
    #[derive(Debug, Default)]
    pub struct WasmClipboard {
        content: Mutex<Option<ClipboardContent>>,
    }

    impl WasmClipboard {
        /// Create a new empty WASM clipboard backend.
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl RichClipboardBackend for WasmClipboard {
        fn set_contents(&self, content: ClipboardContent) -> bool {
            // Stored for same-session reads, but this is not the browser clipboard.
            *lock(&self.content) = Some(content);
            false
        }

        fn get_contents(&self) -> Option<ClipboardContent> {
            lock(&self.content).clone()
        }

        fn has_format(&self, content_type: &str) -> bool {
            lock(&self.content).as_ref().is_some_and(|c| c.content_type() == content_type)
        }
    }
}
