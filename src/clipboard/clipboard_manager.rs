#[cfg(not(feature = "mini"))]
use crate::platform::get_platform;
use crate::platform::Platform;
/// High-level clipboard access facade.
///
/// This manager forwards clipboard operations to the active platform backend.
pub struct ClipboardManager;
impl ClipboardManager {
    /// Set plain text into the system/process clipboard.
    ///
    /// Returns `true` when the backend accepts the update.
    #[cfg(not(feature = "mini"))]
    pub fn set_text(text: impl AsRef<str>) -> bool {
        Self::set_text_with(get_platform(), text.as_ref())
    }
    /// Clipboard not available in mini mode.
    #[cfg(feature = "mini")]
    pub fn set_text(_text: impl AsRef<str>) -> bool {
        false
    }
    /// Read plain text from the clipboard.
    ///
    /// Returns an empty string when no text is available.
    #[cfg(not(feature = "mini"))]
    pub fn text() -> String {
        Self::text_with(get_platform())
    }
    /// Clipboard not available in mini mode.
    #[cfg(feature = "mini")]
    pub fn text() -> String {
        String::new()
    }
    pub(crate) fn set_text_with(platform: &dyn Platform, text: &str) -> bool {
        platform.set_clipboard_text(text)
    }
    pub(crate) fn text_with(platform: &dyn Platform) -> String {
        platform.get_clipboard_text()
    }
}
