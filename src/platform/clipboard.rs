// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Rich clipboard content types and backend trait.
//!
//! Extends the existing plain-text clipboard with support for HTML, RTF,
//! images, file lists, and application-defined (custom) MIME types. Each
//! platform backend can implement the `RichClipboardBackend` trait to provide
//! native clipboard integration.

use crate::compat::lock;
use crate::compat::String;
use crate::compat::ToString;
use crate::compat::Vec;
#[cfg(not(alloc_frugal))]
use std::path::PathBuf;

/// Content types that can be stored on the system clipboard.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardContent {
    /// Plain UTF-8 text.
    Text(String),
    /// HTML content with optional plain-text fallback.
    Html {
        /// The HTML markup itself.
        html: String,
        /// Plain-text rendering of `html`, offered to consumers that cannot
        /// handle markup. Not derived from `html`; the caller keeps the two in
        /// sync.
        plain: String,
    },
    /// Rich Text Format content.
    ///
    /// Raw RTF bytes, including the `{\rtf1` header. Stored as bytes rather than
    /// a `String` because the format is byte-oriented and not guaranteed UTF-8.
    Rtf(Vec<u8>),
    /// RGBA image data.
    Image {
        /// Image width in pixels.
        width: u32,
        /// Image height in pixels.
        height: u32,
        /// Row-major pixel data, 4 bytes per pixel in R, G, B, A order. Expected
        /// to be `width * height * 4` bytes long; the length is not validated
        /// here, so a mismatched buffer is stored as given.
        data: Vec<u8>,
    },
    /// List of file URLs.
    #[cfg(not(alloc_frugal))]
    Files(Vec<PathBuf>),
    /// Application-defined content, carried under its own MIME type.
    ///
    /// # Why an application needs its own type
    ///
    /// The five fixed kinds above cover what a *general* clipboard carries. An application that
    /// copies something of its own — a node subtree, a resource reference — has a payload whose
    /// type only it understands. Without a variant for that payload the only route left is to
    /// wrap it in text and put it on the `text/plain` channel, which loses the type at both ends:
    /// the reader cannot tell "the user copied ordinary text" from "my own payload" without
    /// sniffing the body, and no other application can negotiate for the type by name. This
    /// variant carries the type verbatim, so `has_format`/`content_type` answer it truthfully and
    /// a paste can ask for `application/vnd.example.payload+json` by name.
    ///
    /// `data` is opaque to this crate: it is stored and returned byte-for-byte and is never
    /// interpreted, because the type named in `content_type` is what defines its meaning.
    Custom {
        /// The payload's MIME type, e.g. `application/vnd.example.payload+json`.
        content_type: String,
        /// The payload bytes, stored and returned unmodified.
        data: Vec<u8>,
    },
}

impl ClipboardContent {
    /// Returns the MIME type this content is carried under.
    ///
    /// The return type is `&str` rather than `&'static str` because [`Self::Custom`] answers with
    /// the type the caller supplied, which is not known at compile time.
    pub fn content_type(&self) -> &str {
        match self {
            Self::Text(_) => "text/plain",
            Self::Html { .. } => "text/html",
            Self::Rtf(_) => "text/rtf",
            Self::Image { .. } => "image/png",
            Self::Custom { content_type, .. } => content_type,
            #[cfg(not(alloc_frugal))]
            Self::Files(_) => "text/uri-list",
        }
    }

    /// The content's bytes, when its type has a byte-for-byte representation here.
    ///
    /// `None` for the two kinds where returning bytes would mislabel them:
    ///
    /// * [`Self::Image`]'s bytes are row-major RGBA, whereas its `content_type` is `image/png` —
    ///   handing the RGBA buffer back as "the `image/png` bytes" would be a false claim about the
    ///   encoding;
    /// * [`Self::Files`] would have to be encoded as a `text/uri-list`, which is a set of *URIs*
    ///   and a claim about how a path should be spelled that this crate does not make.
    pub(crate) fn raw_bytes(&self) -> Option<Vec<u8>> {
        match self {
            Self::Text(text) => Some(text.as_bytes().to_vec()),
            Self::Html { html, .. } => Some(html.as_bytes().to_vec()),
            Self::Rtf(bytes) => Some(bytes.clone()),
            Self::Custom { data, .. } => Some(data.clone()),
            Self::Image { .. } => None,
            #[cfg(not(alloc_frugal))]
            Self::Files(_) => None,
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
///
/// The fixed kinds (`Text`/`Html`/`Rtf`/`Image`/`Files`) go through
/// [`Self::set_contents`]/[`Self::get_contents`]; an application-defined type goes through
/// [`Self::set_contents_with_type`]/[`Self::get_contents_for_type`] and is negotiable by name.
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

    /// Store `data` on the clipboard under an application-defined MIME type.
    ///
    /// # Why this is on the trait and not just a `ClipboardContent::Custom` a caller writes
    ///
    /// The byte-level typed entry is what an application that copies its own payload actually
    /// needs, and a backend that can carry a custom type natively (an X11/Wayland selection target,
    /// an `NSPasteboard` custom type) implements it directly instead of round-tripping through a
    /// structured [`ClipboardContent`]. The default routes through [`Self::set_contents`], so a
    /// backend that does not override it inherits the honest answer for its carry set: the write is
    /// refused (`false`) where the backend does not carry custom types.
    fn set_contents_with_type(&self, content_type: &str, data: &[u8]) -> bool {
        self.set_contents(ClipboardContent::Custom {
            content_type: content_type.to_string(),
            data: data.to_vec(),
        })
    }

    /// Read the bytes the clipboard holds under `content_type`, or `None` when it holds none.
    ///
    /// # What this answers for
    ///
    /// The fixed kinds this crate ships (`text/plain`, `text/html`, `text/rtf`) and any
    /// application-defined type a writer put there: for each, the bytes are the content's own
    /// bytes (`ClipboardContent::raw_bytes`). [`ClipboardContent::Image`] and
    /// [`ClipboardContent::Files`] answer `None` for the reason recorded on that method — their
    /// MIME type does not name the bytes this crate would have to return.
    ///
    /// The default derives the answer from [`Self::get_contents`], which is the honest reading for
    /// a backend whose only source is that structured accessor; a backend with a native typed
    /// channel overrides this to answer by type directly.
    fn get_contents_for_type(&self, content_type: &str) -> Option<Vec<u8>> {
        let content = self.get_contents()?;
        if content.content_type() != content_type {
            return None;
        }
        content.raw_bytes()
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
    /// Creates an empty mock clipboard. Equivalent to [`MockClipboard::default`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl RichClipboardBackend for MockClipboard {
    fn set_contents(&self, content: ClipboardContent) -> bool {
        *lock(&self.content) = Some(content);
        true
    }

    fn get_contents(&self) -> Option<ClipboardContent> {
        lock(&self.content).clone()
    }

    fn has_format(&self, content_type: &str) -> bool {
        lock(&self.content).as_ref().is_some_and(|c| c.content_type() == content_type)
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

    // Spawning a thread is meaningless on wasm32 (single-threaded); the
    // Send requirement is verified on the host where threads exist.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
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
    #[cfg(not(alloc_frugal))]
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

    /// An application-defined type must round-trip by name, byte-for-byte.
    ///
    /// This is the request in `docs/issues/feedback-20261015.md` §十一: a designer that copies its
    /// own payload needs the clipboard to carry *its* MIME type, not to smuggle the bytes through
    /// `text/plain`. The assertion covers both ends the designer needs — the type is readable back
    /// (`has_format`/`content_type`) and the payload is unchanged.
    #[test]
    fn test_clipboard_custom_type_roundtrips_by_name() {
        const NODES: &str = "application/vnd.rustage.nodes+json";
        let clip = MockClipboard::new();
        let payload = br#"{"vendor":"rustage","kind":"nodes"}"#.to_vec();

        assert!(clip.set_contents_with_type(NODES, &payload));
        assert!(clip.has_format(NODES), "the custom type must be negotiable by name");
        assert_eq!(clip.get_contents_for_type(NODES), Some(payload.clone()));
        assert_eq!(
            clip.get_contents(),
            Some(ClipboardContent::Custom { content_type: NODES.into(), data: payload.clone() })
        );
        assert_eq!(clip.get_contents().unwrap().content_type(), NODES);
        // A type the clipboard does not hold must not be answered with the one it does.
        assert_eq!(clip.get_contents_for_type("text/plain"), None);
        assert!(!clip.has_format("text/plain"));
    }

    /// The typed read serves the fixed byte-oriented kinds as well as custom ones.
    ///
    /// `image/png` is deliberately excluded: the carried bytes are RGBA, so returning them as the
    /// `image/png` payload would be a false claim about the encoding (see `raw_bytes`).
    #[test]
    fn test_get_contents_for_type_serves_fixed_kinds_but_not_mislabelled_ones() {
        let clip = MockClipboard::new();
        clip.set_contents(ClipboardContent::Text("hello".into()));
        assert_eq!(clip.get_contents_for_type("text/plain"), Some(b"hello".to_vec()));
        assert_eq!(clip.get_contents_for_type("text/html"), None);

        clip.set_contents(ClipboardContent::Image {
            width: 1,
            height: 1,
            data: vec![0, 0, 0, 255],
        });
        assert!(clip.has_format("image/png"));
        assert_eq!(clip.get_contents_for_type("image/png"), None);
    }

    /// The format answers on the fixed types keep working after `content_type` became `&str`.
    #[test]
    fn test_fixed_content_types_are_unchanged() {
        assert_eq!(ClipboardContent::Text("a".into()).content_type(), "text/plain");
        assert_eq!(
            ClipboardContent::Html { html: String::new(), plain: String::new() }.content_type(),
            "text/html"
        );
        assert_eq!(ClipboardContent::Rtf(Vec::new()).content_type(), "text/rtf");
        assert_eq!(
            ClipboardContent::Image { width: 0, height: 0, data: Vec::new() }.content_type(),
            "image/png"
        );
    }
}
