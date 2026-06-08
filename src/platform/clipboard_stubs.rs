//! Platform-specific rich clipboard stubs.
//! These will be replaced with real platform clipboard bindings.

#[cfg(target_os = "macos")]
pub mod macos {
    //! macOS clipboard stub — will use NSPasteboard.
    //! Reference: NSPasteboard, NSPasteboardItem, NSPasteboardItemDataProvider

    use super::super::clipboard::{ClipboardContent, RichClipboardBackend};

    pub struct MacOsClipboard;

    impl RichClipboardBackend for MacOsClipboard {
        fn set_contents(&self, _content: ClipboardContent) -> bool {
            log::info!("[macOS clipboard] set_contents: placeholder");
            false
        }
        fn get_contents(&self) -> Option<ClipboardContent> {
            log::info!("[macOS clipboard] get_contents: placeholder");
            None
        }
        fn has_format(&self, content_type: &str) -> bool {
            log::info!("[macOS clipboard] has_format({:?}): placeholder", content_type);
            false
        }
    }
}

#[cfg(target_os = "windows")]
pub mod windows {
    //! Windows clipboard stub — will use Win32 clipboard API.
    //! Reference: OpenClipboard, SetClipboardData, CF_HTML, CF_RTF

    use super::super::clipboard::{ClipboardContent, RichClipboardBackend};

    pub struct WindowsClipboard;

    impl RichClipboardBackend for WindowsClipboard {
        fn set_contents(&self, _content: ClipboardContent) -> bool {
            log::info!("[Windows clipboard] set_contents: placeholder");
            false
        }
        fn get_contents(&self) -> Option<ClipboardContent> {
            log::info!("[Windows clipboard] get_contents: placeholder");
            None
        }
        fn has_format(&self, content_type: &str) -> bool {
            log::info!("[Windows clipboard] has_format({:?}): placeholder", content_type);
            false
        }
    }
}
