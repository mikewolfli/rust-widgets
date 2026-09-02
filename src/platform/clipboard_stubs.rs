//! Platform-specific rich clipboard stubs.
//! These will be replaced with real platform clipboard bindings.

#[cfg(all(target_os = "macos", feature = "macos-legacy"))]
pub mod macos {
    //! Real macOS clipboard using NSPasteboard rich content APIs.
    //! Reference: NSPasteboard, NSPasteboardItem, NSPasteboardItemDataProvider

    use super::super::clipboard::{ClipboardContent, RichClipboardBackend};
    use cocoa::base::{id, nil, BOOL, YES};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};

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
            let str_id: id = msg_send![item, stringForType: NSString::alloc(nil).init_str("public.utf8-plain-text")];
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
                        let ns_string = NSString::alloc(nil).init_str(text);
                        let success: BOOL = msg_send![item, setString: ns_string forType: NSString::alloc(nil).init_str("public.utf8-plain-text")];
                        if success == YES {
                            let arr: id = msg_send![class!(NSArray), arrayWithObject: item];
                            let _: BOOL = msg_send![pb, writeObjects: arr];
                            true
                        } else {
                            false
                        }
                    }
                    ClipboardContent::Html { html, plain } => {
                        let item: id = msg_send![class!(NSPasteboardItem), alloc];
                        let item: id = msg_send![item, init];

                        let ns_html = NSString::alloc(nil).init_str(html);
                        let html_ok: BOOL = msg_send![item, setString: ns_html forType: NSString::alloc(nil).init_str("public.html")];

                        let ns_plain = NSString::alloc(nil).init_str(plain);
                        let plain_ok: BOOL = msg_send![item, setString: ns_plain forType: NSString::alloc(nil).init_str("public.utf8-plain-text")];

                        if html_ok == YES || plain_ok == YES {
                            let arr: id = msg_send![class!(NSArray), arrayWithObject: item];
                            let _: BOOL = msg_send![pb, writeObjects: arr];
                            true
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
                let html_id: id =
                    msg_send![item, stringForType: NSString::alloc(nil).init_str("public.html")];
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
                let ns_type = NSString::alloc(nil).init_str(content_type);
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
    use winapi::shared::minwindef::{FALSE, UINT};
    use winapi::um::winbase::GlobalAlloc;
    use winapi::um::winbase::{GlobalLock, GlobalSize, GlobalUnlock, GHND};
    use winapi::um::winuser::CF_UNICODETEXT;
    use winapi::um::winuser::{
        CloseClipboard, GetClipboardData, OpenClipboard, RegisterClipboardFormatA, SetClipboardData,
    };

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
        fn format_cf_html(html: &str) -> Vec<u8> {
            let fragment = html;
            let full_html = format!(
                "<html><body><!--StartFragment-->{fragment}<!--EndFragment--></body></html>"
            );

            // The fragment starts right after <!--StartFragment-->
            let start_fragment_offset = "<!--StartFragment-->".len();
            let end_fragment_offset = start_fragment_offset + fragment.len();

            // Build header with placeholder offsets to calculate the final header length
            let placeholder = "0000000000";
            let header_template = format!(
                "Version:0.9\r\nStartHTML:{placeholder}\r\nEndHTML:{placeholder}\r\nStartFragment:{placeholder}\r\nEndFragment:{placeholder}\r\n"
            );

            let start_html = header_template.len();
            let end_html = start_html + full_html.len();
            let start_fragment = start_html + start_fragment_offset;
            let end_fragment = start_html + end_fragment_offset;

            let result = format!(
                "Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\nStartFragment:{start_fragment:010}\r\nEndFragment:{end_fragment:010}\r\n{full_html}"
            );

            result.into_bytes()
        }

        /// Parse a CF_HTML formatted string and extract the HTML fragment.
        fn parse_cf_html(cf_html: &str) -> String {
            // Parse StartFragment and EndFragment from the header
            let mut start_fragment = 0usize;
            let mut end_fragment = 0usize;

            for line in cf_html.lines() {
                if let Some(val) = line.strip_prefix("StartFragment:") {
                    start_fragment = val.trim().parse().unwrap_or(0);
                } else if let Some(val) = line.strip_prefix("EndFragment:") {
                    end_fragment = val.trim().parse().unwrap_or(0);
                }
            }

            if start_fragment > 0 && end_fragment > start_fragment && end_fragment <= cf_html.len()
            {
                cf_html[start_fragment..end_fragment].to_string()
            } else {
                // Fallback: try to find the fragment markers
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
                        return !result.is_null();
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
    //! macOS clipboard using objc2 NSPasteboard APIs.
    //! Uses objc2 runtime messaging with NSPasteboard, NSPasteboardItem, and NSArray.

    use super::super::clipboard::{ClipboardContent, RichClipboardBackend};
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
                            let arr: *mut AnyObject =
                                msg_send![class!(NSArray), arrayWithObject: &*ns_string];
                            let _: bool = msg_send![pb, writeObjects: arr];
                            true
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
                            let arr: *mut AnyObject =
                                msg_send![class!(NSArray), arrayWithObject: &*ns_html];
                            let _: bool = msg_send![pb, writeObjects: arr];
                            true
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

// ── Linux clipboard (in-memory mock) ──

/// Linux clipboard backend (in-memory mock).
///
/// Uses a simple in-memory store since native Linux clipboard access
/// requires platform-specific libraries (GTK / Wayland / X11).
///
/// Stores clipboard contents in memory. This provides a functional
/// clipboard for testing and environments without a desktop session.
/// Real Linux clipboard integration can be added later via GTK or
/// Wayland data-device protocols.
#[cfg(target_os = "linux")]
pub mod linux {

    use super::super::clipboard::{ClipboardContent, RichClipboardBackend};
    use crate::compat::Mutex;

    /// In-memory clipboard backend for Linux.
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
            *self.content.lock().unwrap() = Some(content);
            true
        }

        fn get_contents(&self) -> Option<ClipboardContent> {
            self.content.lock().unwrap().clone()
        }

        fn has_format(&self, content_type: &str) -> bool {
            self.content.lock().unwrap().as_ref().is_some_and(|c| c.content_type() == content_type)
        }
    }
}

// ── WASM clipboard (in-memory mock) ──

/// WASM/WebAssembly clipboard backend (in-memory mock).
///
/// The browser `navigator.clipboard` API is entirely Promise-based, making it
/// unsuitable for synchronous trait methods, so this backend uses a simple
/// in-memory store that is fully functional within a single WASM session.
#[cfg(feature = "wasm")]
pub mod wasm {
    use super::super::clipboard::{ClipboardContent, RichClipboardBackend};
    use crate::compat::Mutex;

    /// In-memory clipboard backend for WASM.
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
            *self.content.lock().unwrap() = Some(content);
            true
        }

        fn get_contents(&self) -> Option<ClipboardContent> {
            self.content.lock().unwrap().clone()
        }

        fn has_format(&self, content_type: &str) -> bool {
            self.content.lock().unwrap().as_ref().is_some_and(|c| c.content_type() == content_type)
        }
    }
}
