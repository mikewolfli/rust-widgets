//! Platform-specific rich clipboard stubs.
//! These will be replaced with real platform clipboard bindings.

#[cfg(target_os = "macos")]
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
            // Only handle Text format for now; other formats (Html, Rtf, Image, Files)
            // are not yet supported via NSPasteboardItem.
            let text = match &content {
                ClipboardContent::Text(t) => t.clone(),
                _ => {
                    log::warn!("[macOS clipboard] non-text format not yet supported");
                    return false;
                }
            };
            let result = std::panic::catch_unwind(|| unsafe {
                let pb = Self::prepare_pasteboard();
                let item: id = msg_send![class!(NSPasteboardItem), alloc];
                let item: id = msg_send![item, init];
                let ns_string = NSString::alloc(nil).init_str(&text);
                let success: BOOL = msg_send![item, setString: ns_string forType: NSString::alloc(nil).init_str("public.utf8-plain-text")];
                if success == YES {
                    let arr: id = msg_send![class!(NSArray), arrayWithObject: item];
                    let _: BOOL = msg_send![pb, writeObjects: arr];
                    true
                } else {
                    false
                }
            });
            result.unwrap_or(false)
        }

        fn get_contents(&self) -> Option<ClipboardContent> {
            let result = std::panic::catch_unwind(|| unsafe {
                let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
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
    use winapi::um::winbase::{GlobalLock, GlobalUnlock, GHND};
    use winapi::um::winuser::CF_UNICODETEXT;
    use winapi::um::winuser::{CloseClipboard, GetClipboardData, OpenClipboard, SetClipboardData};

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
            let wide_slice = std::slice::from_raw_parts(ptr as *const u16, 1024);
            let nul_pos = wide_slice.iter().position(|&c| c == 0).unwrap_or(0);
            let result = String::from_utf16_lossy(&wide_slice[..nul_pos]);
            GlobalUnlock(h_mem);
            Some(result)
        }
    }

    impl RichClipboardBackend for WindowsClipboard {
        fn set_contents(&self, content: ClipboardContent) -> bool {
            let text = match &content {
                ClipboardContent::Text(t) => t.clone(),
                _ => {
                    log::warn!("[Windows clipboard] non-text format not yet supported");
                    return false;
                }
            };
            let result = std::panic::catch_unwind(|| unsafe {
                if OpenClipboard(std::ptr::null_mut()) == FALSE {
                    return false;
                }
                let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
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
            });
            result.unwrap_or(false)
        }

        fn get_contents(&self) -> Option<ClipboardContent> {
            let result = std::panic::catch_unwind(|| unsafe {
                if OpenClipboard(std::ptr::null_mut()) == FALSE {
                    return None;
                }
                let text = Self::read_unicode_text();
                CloseClipboard();
                text.map(ClipboardContent::Text)
            });
            result.unwrap_or(None)
        }

        fn has_format(&self, content_type: &str) -> bool {
            let win_format =
                if content_type == "text/plain" || content_type == "public.utf8-plain-text" {
                    CF_UNICODETEXT
                } else {
                    return false;
                };
            let result = std::panic::catch_unwind(|| unsafe {
                let fmt: UINT = win_format;
                let h = GetClipboardData(fmt);
                !h.is_null()
            });
            result.unwrap_or(false)
        }
    }
}
