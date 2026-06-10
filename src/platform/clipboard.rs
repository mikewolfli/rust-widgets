//! Rich clipboard content types and backend trait.
//!
//! Extends the existing plain-text clipboard with support for HTML, RTF,
//! images, and file lists. Each platform backend can implement the
//! `RichClipboardBackend` trait to provide native clipboard integration.

#[cfg(not(feature = "mini"))]
use std::path::PathBuf;

/// Content types that can be stored on the system clipboard.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardContent {
    /// Plain UTF-8 text.
    Text(String),
    /// HTML content with optional plain-text fallback.
    Html { html: String, plain: String },
    /// Rich Text Format content.
    Rtf(Vec<u8>),
    /// RGBA image data.
    Image { width: u32, height: u32, data: Vec<u8> },
    /// List of file URLs.
    Files(Vec<PathBuf>),
}

impl ClipboardContent {
    /// Returns a human-readable description of the content type.
    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Text(_) => "text/plain",
            Self::Html { .. } => "text/html",
            Self::Rtf(_) => "text/rtf",
            Self::Image { .. } => "image/png",
            Self::Files(_) => "text/uri-list",
        }
    }
}

impl From<String> for ClipboardContent {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&str> for ClipboardContent {
    fn from(text: &str) -> Self {
        Self::Text(text.to_string())
    }
}

/// Rich clipboard backend trait.
///
/// Implement this trait for each platform to provide native clipboard
/// access. The [`MockClipboard`] implementation is used for testing.
pub trait RichClipboardBackend: Send + Sync {
    /// Store content on the system clipboard, replacing any existing content.
    fn set_contents(&self, content: ClipboardContent) -> bool;

    /// Retrieve content from the system clipboard if it matches a supported format.
    fn get_contents(&self) -> Option<ClipboardContent>;

    /// Check whether the clipboard currently contains data in any of the
    /// given content types.
    fn has_format(&self, content_type: &str) -> bool;

    /// Set HTML content on the clipboard (R2.7).
    fn set_clipboard_html(&self, html: &str, plain_text: &str) {
        self.set_contents(ClipboardContent::Html {
            html: html.to_string(),
            plain: plain_text.to_string(),
        });
    }

    /// Set an image on the clipboard from raw RGBA data (R2.7).
    fn set_clipboard_image(&self, rgba: &[u8], width: u32, height: u32) {
        self.set_contents(ClipboardContent::Image { width, height, data: rgba.to_vec() });
    }
}

/// Mock clipboard backend for testing.
///
/// Stores clipboard contents in memory, enabling round-trip tests
/// without accessing the OS clipboard.
#[derive(Debug, Default)]
pub struct MockClipboard {
    content: crate::compat::Mutex<Option<ClipboardContent>>,
}

impl MockClipboard {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RichClipboardBackend for MockClipboard {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_set_get_text() {
        let clip = MockClipboard::new();
        assert!(clip.set_contents(ClipboardContent::Text("hello".into())));
        assert_eq!(clip.get_contents(), Some(ClipboardContent::Text("hello".into())));
    }

    #[test]
    fn test_mock_set_get_html() {
        let clip = MockClipboard::new();
        let html = ClipboardContent::Html { html: "<b>bold</b>".into(), plain: "bold".into() };
        assert!(clip.set_contents(html.clone()));
        assert_eq!(clip.get_contents(), Some(html));
    }

    #[test]
    fn test_mock_has_format() {
        let clip = MockClipboard::new();
        clip.set_contents(ClipboardContent::Text("test".into()));
        assert!(clip.has_format("text/plain"));
        assert!(!clip.has_format("text/html"));
    }

    #[test]
    fn test_clipboard_content_from_string() {
        let c: ClipboardContent = "hello".into();
        assert_eq!(c, ClipboardContent::Text("hello".into()));
    }

    #[test]
    fn test_clipboard_content_type() {
        assert_eq!(ClipboardContent::Text("a".into()).content_type(), "text/plain");
        assert_eq!(
            ClipboardContent::Html { html: "".into(), plain: "".into() }.content_type(),
            "text/html"
        );
        assert_eq!(ClipboardContent::Rtf(vec![]).content_type(), "text/rtf");
    }

    #[test]
    fn test_mock_initial_empty() {
        let clip = MockClipboard::new();
        assert_eq!(clip.get_contents(), None);
        assert!(!clip.has_format("text/plain"));
    }

    #[test]
    fn test_clipboard_mock_thread_safety() {
        let clip = alloc::sync::Arc::new(MockClipboard::new());
        let clip2 = clip.clone();
        std::thread::spawn(move || {
            clip2.set_contents(ClipboardContent::Text("from thread".into()))
        })
        .join()
        .unwrap();
        assert_eq!(clip.get_contents(), Some(ClipboardContent::Text("from thread".to_string())));
    }

    #[test]
    fn test_clipboard_files_format() {
        let files = ClipboardContent::Files(vec!["/tmp/a.txt".into(), "/tmp/b.txt".into()]);
        assert_eq!(files.content_type(), "text/uri-list");
    }

    #[test]
    fn test_clipboard_image_roundtrip() {
        let clip = MockClipboard::new();
        let img = ClipboardContent::Image {
            width: 2,
            height: 2,
            data: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255],
        };
        clip.set_contents(img.clone());
        assert_eq!(clip.get_contents(), Some(img));
        assert!(clip.has_format("image/png"));
    }
}
