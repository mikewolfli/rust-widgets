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
    // `String` and `Vec` are both used (the typed read returns `Vec<u8>`);
    // `CStr::to_string_lossy` resolves inherently.
    use crate::compat::{String, Vec};
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

    /// The pasteboard type name for one of this crate's MIME spellings.
    ///
    /// `text/plain` and `text/html` are the crate's MIME names; `NSPasteboard` calls the same two
    /// channels by their UTIs. Every other type — including an application-defined one such as
    /// `application/vnd.example.payload+json` — is used verbatim, which is exactly what makes a
    /// custom type negotiable by name instead of smuggled through `text/plain`.
    fn pasteboard_type_name(content_type: &str) -> &str {
        match content_type {
            "text/plain" => "public.utf8-plain-text",
            "text/html" => "public.html",
            other => other,
        }
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
                    // An application-defined (or any other) type is carried under its own type
                    // name, verbatim.
                    //
                    // # Why this arm does not use the `NSPasteboardItem`/`writeObjects:` pair
                    //
                    // The item API validates the type as a **UTI** and refuses an application MIME
                    // name: `setData:forType:` on `NSPasteboardItem` answers `NO` (and AppKit logs
                    // "not a valid UTI string") for `application/vnd.example.nodes+json`, because
                    // the `/` and `+` are not UTI syntax. The pasteboard's own
                    // `declareTypes:owner:`/`setData:forType:` pair accepts an arbitrary type string
                    // and round-trips it (measured on macOS 15: `setData:` `YES`, `dataForType:`
                    // returns the bytes), which is the mechanism that makes the caller's type name
                    // negotiable exactly as written.
                    ClipboardContent::Custom { content_type, data } => {
                        let ty = nsstring(content_type);
                        let types: id = msg_send![class!(NSArray), arrayWithObject: ty];
                        let _: i64 = msg_send![pb, declareTypes: types owner: nil];
                        let ns_data: id = msg_send![
                            class!(NSData),
                            dataWithBytes: data.as_ptr() as *const std::os::raw::c_void
                            length: data.len()
                        ];
                        let set_ok: BOOL = msg_send![pb, setData: ns_data forType: ty];
                        set_ok == YES
                    }
                    other => {
                        log::warn!(
                            "[macOS clipboard] {} is not carried by this backend; the write is \
                             reported as refused rather than pretended",
                            other.content_type()
                        );
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
                let ns_type = nsstring(pasteboard_type_name(content_type));
                let arr: id = msg_send![class!(NSArray), arrayWithObject: ns_type];
                let available: id = msg_send![pb, availableTypeFromArray: arr];
                available != nil
            });
            result.unwrap_or(false)
        }

        /// Reads by type directly, rather than from [`Self::get_contents`].
        ///
        /// The structured accessor only recognizes the two kinds this backend writes itself
        /// (`text`/`html`); a custom type has to be asked for by name. The two fixed byte-oriented
        /// kinds are read through their pasteboard UTIs, while every other type goes through the
        /// pasteboard's own `dataForType:`, which is the accessor that carries an arbitrary type
        /// string verbatim (see `set_contents`' `Custom` arm).
        fn get_contents_for_type(&self, content_type: &str) -> Option<Vec<u8>> {
            let result = std::panic::catch_unwind(|| unsafe {
                let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
                match content_type {
                    // The two the backend writes itself, read through the item that holds them.
                    "text/plain" => Self::read_plain_text(pb).map(String::into_bytes),
                    "text/html" => {
                        let items: id = msg_send![pb, pasteboardItems];
                        let count: usize = msg_send![items, count];
                        if count == 0 {
                            return None;
                        }
                        let item: id = msg_send![items, objectAtIndex: 0u64];
                        let html_id: id = msg_send![item, stringForType: nsstring("public.html")];
                        if html_id == nil {
                            return None;
                        }
                        let c_str: *const std::os::raw::c_char = msg_send![html_id, UTF8String];
                        if c_str.is_null() {
                            return None;
                        }
                        Some(
                            std::ffi::CStr::from_ptr(c_str)
                                .to_string_lossy()
                                .into_owned()
                                .into_bytes(),
                        )
                    }
                    other => {
                        let data: id = msg_send![pb, dataForType: nsstring(other)];
                        if data == nil {
                            return None;
                        }
                        let len: usize = msg_send![data, length];
                        let bytes: *const u8 = msg_send![data, bytes];
                        if bytes.is_null() {
                            return None;
                        }
                        // `NSData`'s buffer is valid for the object's lifetime, which is this
                        // call's; the bytes are copied out before it is released.
                        Some(std::slice::from_raw_parts(bytes, len).to_vec())
                    }
                }
            });
            result.unwrap_or(None)
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
    use crate::compat::{String, Vec};
    use winapi::shared::minwindef::{FALSE, UINT};
    use winapi::um::winbase::GlobalAlloc;
    use winapi::um::winbase::{GlobalFree, GlobalLock, GlobalSize, GlobalUnlock, GHND};
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

        /// The CF_HTML fragment the clipboard holds, or `None` when it holds none.
        ///
        /// The clipboard must already be open: `GetClipboardData` is only valid between
        /// `OpenClipboard` and `CloseClipboard`, and this leaves that ownership with the caller.
        unsafe fn read_html_fragment() -> Option<String> {
            let cf_html_format = Self::html_format_id();
            let h_html = GetClipboardData(cf_html_format);
            if h_html.is_null() {
                return None;
            }
            let ptr = GlobalLock(h_html);
            if ptr.is_null() {
                return None;
            }
            let byte_size = GlobalSize(h_html);
            let slice = std::slice::from_raw_parts(ptr as *const u8, byte_size as usize);
            // The document is a NUL-terminated string; anything after the terminator is padding.
            let nul_pos = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
            let cf_html_str = String::from_utf8_lossy(&slice[..nul_pos]).into_owned();
            GlobalUnlock(h_html);
            Some(Self::parse_cf_html(&cf_html_str))
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

        /// The registered Win32 clipboard format id for an application-defined MIME type.
        ///
        /// `RegisterClipboardFormatA` names any string and returns the same id for the same name
        /// for the life of the process, so this is the Win32 mechanism a custom type is carried
        /// under (the CF_* constants only cover the formats Windows itself defines). A name that
        /// cannot be encoded as a NUL-terminated C string, or one whose registration fails,
        /// answers `0`, which no real format id is (registered ids start at `0xC000`).
        unsafe fn registered_format_id(content_type: &str) -> UINT {
            let Ok(name) = std::ffi::CString::new(content_type) else {
                return 0;
            };
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
                            // `SetClipboardData` did **not** take ownership, so the block
                            // allocated above is still ours to free. Without this the
                            // clipboard being busy (or the format being refused) leaked one
                            // `GHND` block per failed write.
                            GlobalFree(h_html);
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
                            // Same ownership rule as the HTML block above: only a successful
                            // `SetClipboardData` transfers the handle to the system.
                            GlobalFree(h_text);
                            success = false;
                        }

                        CloseClipboard();
                        success
                    }
                    ClipboardContent::Custom { content_type, data } => {
                        // The application-defined type is carried under a clipboard format
                        // registered by its MIME name — the Win32 mechanism for a type Windows
                        // does not itself define through a `CF_*` constant. This is what makes the
                        // type negotiable by name instead of smuggled through `text/plain`.
                        let format = Self::registered_format_id(content_type);
                        if format == 0 {
                            CloseClipboard();
                            return false;
                        }
                        // `GlobalAlloc` of a zero-byte payload may answer null; an empty payload
                        // is still a legitimate write, so at least one byte is allocated for it.
                        let h_mem = GlobalAlloc(GHND, data.len().max(1));
                        if h_mem.is_null() {
                            CloseClipboard();
                            return false;
                        }
                        let ptr = GlobalLock(h_mem);
                        if ptr.is_null() {
                            GlobalFree(h_mem);
                            CloseClipboard();
                            return false;
                        }
                        if !data.is_empty() {
                            std::ptr::copy_nonoverlapping(
                                data.as_ptr(),
                                ptr as *mut u8,
                                data.len(),
                            );
                        }
                        GlobalUnlock(h_mem);
                        let set = SetClipboardData(format, h_mem);
                        if set.is_null() {
                            // Only a successful `SetClipboardData` transfers ownership to the
                            // system, so a refused write is ours to free rather than leak.
                            GlobalFree(h_mem);
                            CloseClipboard();
                            return false;
                        }
                        CloseClipboard();
                        true
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
                if let Some(html_content) = Self::read_html_fragment() {
                    let plain = Self::read_unicode_text().unwrap_or_default();
                    CloseClipboard();
                    return Some(ClipboardContent::Html { html: html_content, plain });
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
                        // Any application-defined type is a registered clipboard format, so it is
                        // asked for by name. A format no writer ever registered answers `false`,
                        // which is the honest "this type is not on the clipboard".
                        let format = Self::registered_format_id(content_type);
                        format != 0 && !GetClipboardData(format).is_null()
                    };
                CloseClipboard();
                has
            });
            result.unwrap_or(false)
        }

        /// Reads by type directly from Win32, rather than from [`Self::get_contents`].
        ///
        /// The structured accessor only recognises the fixed kinds it knows; a custom type has to
        /// be asked for by its registered clipboard format. That typed channel is the request in
        /// `docs/issues/feedback-20261015.md` §十一, and it is why a designer's own payload
        /// (`application/vnd.rustage.nodes+json`) can be read back by name instead of sniffed out
        /// of `text/plain`. The fixed byte-oriented kinds are served too, so a caller that reads
        /// everything by type gets a uniform answer.
        fn get_contents_for_type(&self, content_type: &str) -> Option<Vec<u8>> {
            let result = std::panic::catch_unwind(|| unsafe {
                if OpenClipboard(std::ptr::null_mut()) == FALSE {
                    return None;
                }
                let answer =
                    if content_type == "text/plain" || content_type == "public.utf8-plain-text" {
                        Self::read_unicode_text().map(String::into_bytes)
                    } else if content_type == "text/html" {
                        // The HTML *fragment*, not the CF_HTML envelope: the same bytes
                        // `ClipboardContent::Html` reports through `raw_bytes`, so the typed read and
                        // the structured read agree on what `text/html` means.
                        Self::read_html_fragment().map(String::into_bytes)
                    } else {
                        let format = Self::registered_format_id(content_type);
                        if format == 0 {
                            None
                        } else {
                            let h_mem = GetClipboardData(format);
                            if h_mem.is_null() {
                                None
                            } else {
                                let ptr = GlobalLock(h_mem);
                                if ptr.is_null() {
                                    None
                                } else {
                                    let size = GlobalSize(h_mem) as usize;
                                    let bytes =
                                        std::slice::from_raw_parts(ptr as *const u8, size).to_vec();
                                    GlobalUnlock(h_mem);
                                    Some(bytes)
                                }
                            }
                        }
                    };
                CloseClipboard();
                answer
            });
            result.unwrap_or(None)
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
    // that prelude. The typed read returns `Vec<u8>`, so `Vec` comes from the same
    // bridge. `CStr::to_string_lossy` resolves inherently, so its name needs no import.
    use crate::compat::{String, Vec};
    use objc2::class;
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSData, NSString};

    /// The pasteboard type name for one of this crate's MIME spellings.
    ///
    /// `text/plain` and `text/html` are the crate's MIME names; `NSPasteboard` calls the same two
    /// channels by their UTIs. Every other type — including an application-defined one — is used
    /// verbatim, which is what makes a custom type negotiable by name (feedback §十一).
    fn pasteboard_type_name(content_type: &str) -> &str {
        match content_type {
            "text/plain" => "public.utf8-plain-text",
            "text/html" => "public.html",
            other => other,
        }
    }

    /// macOS clipboard backend using objc2 NSPasteboard bindings.
    pub struct MacOsObjc2Clipboard;

    /// Whether the current thread is the AppKit main thread.
    ///
    /// `NSPasteboard` is a window-server singleton, so the same rule the cocoa backend applies
    /// (see `clipboard_stubs::macos`) applies here: the thread that may touch it is the one the
    /// app was started on. The check is what keeps this backend from turning a call from a worker
    /// thread into an AppKit access off the main thread; the caller then gets `false` / `None`,
    /// which is the honest "no host clipboard from here" answer, and its own record is the
    /// fallback.
    pub fn on_main_thread() -> bool {
        objc2::MainThreadMarker::new().is_some()
    }

    /// Puts `text` on the general pasteboard as plain text, and reports the system write.
    ///
    /// A thin wrapper over [`RichClipboardBackend::set_contents`] so that the text path and the
    /// rich path are the *same* code: two implementations of "write text to the clipboard" is
    /// precisely the drift this crate's principles forbid.
    pub fn write_text(text: &str) -> bool {
        if !on_main_thread() {
            return false;
        }
        RichClipboardBackend::set_contents(
            &MacOsObjc2Clipboard,
            ClipboardContent::Text(text.into()),
        )
    }

    /// The general pasteboard's plain text, or `None` when it holds none.
    pub fn read_text() -> Option<String> {
        if !on_main_thread() {
            return None;
        }
        let result = std::panic::catch_unwind(|| unsafe {
            let pb: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
            MacOsObjc2Clipboard::read_plain_text(pb)
        });
        result.unwrap_or(None)
    }

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
            if !on_main_thread() {
                log::debug!(
                    "[macOS objc2 clipboard] refusing a write off the AppKit main thread; the \
                     caller's own record is the fallback"
                );
                return false;
            }
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
                    // An application-defined type travels as `NSData` under its own pasteboard type
                    // name, verbatim.
                    //
                    // # Why this arm does not use the `NSPasteboardItem`/`writeObjects:` pair
                    //
                    // The item API validates the type as a **UTI** and refuses an application MIME
                    // name (`application/vnd.example.nodes+json` has no UTI syntax). The pasteboard's
                    // own `declareTypes:owner:`/`setData:forType:` pair accepts an arbitrary type
                    // string and round-trips it — the same mechanism the cocoa arm documents.
                    ClipboardContent::Custom { content_type, data } => {
                        let ty = NSString::from_str(content_type);
                        let types: *mut AnyObject =
                            msg_send![class!(NSArray), arrayWithObject: &*ty];
                        let _: i64 = msg_send![pb, declareTypes: types, owner: std::ptr::null_mut::<AnyObject>()];
                        let ns_data = NSData::with_bytes(data);
                        let set_ok: bool = msg_send![pb, setData: &*ns_data, forType: &*ty];
                        set_ok
                    }
                    other => {
                        log::warn!(
                            "[macOS objc2 clipboard] {} is not carried by this backend; the write \
                             is reported as refused rather than pretended",
                            other.content_type()
                        );
                        false
                    }
                }
            });
            result.unwrap_or(false)
        }

        fn get_contents(&self) -> Option<ClipboardContent> {
            if !on_main_thread() {
                return None;
            }
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
            if !on_main_thread() {
                return false;
            }
            let result = std::panic::catch_unwind(|| unsafe {
                let pb: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
                let ns_type = NSString::from_str(pasteboard_type_name(content_type));
                let arr: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: &*ns_type];
                let available: *mut AnyObject = msg_send![pb, availableTypeFromArray: arr];
                !available.is_null()
            });
            result.unwrap_or(false)
        }

        /// Reads by type directly, rather than from [`Self::get_contents`].
        ///
        /// Mirrors the cocoa backend's override: the two fixed byte-oriented kinds are read through
        /// their pasteboard UTIs, while every other type goes through the pasteboard's own
        /// `dataForType:`, which carries an arbitrary type string verbatim.
        fn get_contents_for_type(&self, content_type: &str) -> Option<Vec<u8>> {
            if !on_main_thread() {
                return None;
            }
            let result = std::panic::catch_unwind(|| unsafe {
                let pb: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
                match content_type {
                    "text/plain" => Self::read_plain_text(pb).map(String::into_bytes),
                    "text/html" => {
                        let items: *mut AnyObject = msg_send![pb, pasteboardItems];
                        let count: usize = msg_send![items, count];
                        if count == 0 {
                            return None;
                        }
                        let item: *mut AnyObject = msg_send![items, objectAtIndex: 0u64];
                        let html_type = NSString::from_str("public.html");
                        let html_id: *mut AnyObject = msg_send![item, stringForType: &*html_type];
                        if html_id.is_null() {
                            return None;
                        }
                        let c_str: *const std::os::raw::c_char = msg_send![html_id, UTF8String];
                        if c_str.is_null() {
                            return None;
                        }
                        Some(
                            std::ffi::CStr::from_ptr(c_str)
                                .to_string_lossy()
                                .into_owned()
                                .into_bytes(),
                        )
                    }
                    other => {
                        let ns_type = NSString::from_str(other);
                        let data: *mut AnyObject = msg_send![pb, dataForType: &*ns_type];
                        if data.is_null() {
                            return None;
                        }
                        let len: usize = msg_send![data, length];
                        // objc2 checks the selector's return encoding (`-[NSData bytes]` returns
                        // `const void *`, `^v`), so the declared type must be `*const c_void` and
                        // the cast to a byte pointer happens here.
                        let bytes: *const std::os::raw::c_void = msg_send![data, bytes];
                        if bytes.is_null() {
                            return None;
                        }
                        // `NSData`'s buffer is valid for the object's lifetime, which is this
                        // call's; the bytes are copied out before it is released.
                        Some(std::slice::from_raw_parts(bytes as *const u8, len).to_vec())
                    }
                }
            });
            result.unwrap_or(None)
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
#[cfg(unix_desktop)]
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
