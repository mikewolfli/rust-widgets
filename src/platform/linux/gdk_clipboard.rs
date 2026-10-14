// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The Linux/GTK system clipboard, over GDK.
//!
//! # What this closes
//!
//! Both halves of the clipboard were wrong in the same direction:
//!
//! * `set_clipboard_text` published to the GDK clipboard but `get_clipboard_text` read the
//!   in-process `BackendState` record, so what a copy put on the display could not be read back —
//!   the read answered from a mirror that only this process writes.
//! * `clipboard_backend()` was never overridden, so `platform_clipboard()` answered `None` on
//!   Linux even in a `gtk-native` build, and a host could not tell "no clipboard here" from "no
//!   rich clipboard here".
//!
//! This module is the rich half: one GDK-backed [`RichClipboardBackend`], plus the plain-text
//! read/write the backend and the `Platform` methods share, so there is exactly one path to the
//! display clipboard.
//!
//! # Why GDK and not the X11/XWayland selection protocol
//!
//! GDK is already linked in this build (`gtk-native`), it speaks both X11 and XWayland selections,
//! and its `wait_for_*` family runs the nested main loop the asynchronous protocol needs. Writing
//! the protocol by hand would mean reimplementing `TARGETS`/`SELECTION_REQUEST` ownership for no
//! gain, and the owner-callback lifetime problem it brings is the same one GDK already solves.
//!
//! # The main-thread rule
//!
//! Every entry point checks `gtk::is_initialized_main_thread()` first, the same rule the rest of
//! this backend follows (see the `Send` note in `linux/types.rs`): GTK objects may only be touched
//! from the thread that called `gtk::init`. A call from anywhere else answers `None`/`false`
//! rather than touching GTK, which is what lets the caller fall back to its own record.
//!
//! [`RichClipboardBackend`]: crate::platform::clipboard::RichClipboardBackend

use crate::compat::String;
use crate::platform::clipboard::{ClipboardContent, RichClipboardBackend};

/// The MIME type a plain-text payload is published under.
const PLAIN_TEXT: &str = "text/plain";
/// The MIME type an HTML payload is published under.
const HTML: &str = "text/html";
/// The `TargetEntry::info` value that selects the HTML arm of the owner callback.
const HTML_INFO: u32 = 1;
/// The `TargetEntry::info` value that selects the plain-text arm of the owner callback.
const PLAIN_INFO: u32 = 2;

/// Runs `body` with the display's clipboard, or `None` when this thread has no usable one.
fn with_clipboard<R>(body: impl FnOnce(&gtk::Clipboard) -> R) -> Option<R> {
    if !gtk::is_initialized_main_thread() {
        return None;
    }
    let display = gtk::gdk::Display::default()?;
    let clipboard = gtk::Clipboard::default(&display)?;
    Some(body(&clipboard))
}

/// `true` when this build can reach a display clipboard at all.
///
/// Answered from the same two facts the callers check, so the capability flag, the trait methods
/// and this predicate cannot disagree: GTK is initialised on this thread and a default display
/// exists. A `false` here is a real "no clipboard on this host right now" — the interface has
/// `GtkClipboard` but no X display behind it (a headless SSH session, for instance).
pub(crate) fn available() -> bool {
    with_clipboard(|_| ()).is_some()
}

/// Puts `text` on the display clipboard as plain text.
///
/// The answer is a **read-back**: `gtk_clipboard_set_text` returns nothing, and a clipboard the
/// display refused (or a nested main loop that never got the selection) would otherwise be
/// reported as a successful write. GDK owns the selection here, so the read does not need another
/// process to answer it.
///
/// # Why the empty string is confirmed the other way round
///
/// GDK has no "empty string" selection: `set_text("")` leaves the clipboard holding **no text**,
/// and `wait_for_text()` then answers `None` — the same answer `clear()` gives. Measured on a live
/// display during this round (GTK 3, quartz backend):
///
/// ```text
/// after set_text("something"): Some("something")
/// after set_text(""):          None
/// after clear():               None
/// ```
///
/// So for `""` the confirmation is "there is now no text", which is what the caller asked for.
/// Comparing the string back in that case would report a successful write as a failure.
pub(crate) fn set_text(text: &str) -> bool {
    with_clipboard(|clipboard| {
        clipboard.set_text(text);
        // `store()` asks the clipboard manager to keep the value after this process exits.
        // Without it the text vanishes on exit, which is not what a user who pressed Ctrl+C
        // expects — and on a session with no manager it is a no-op, not a failure.
        clipboard.store();
        match clipboard.wait_for_text() {
            Some(stored) => stored.as_str() == text,
            None => text.is_empty(),
        }
    })
    .unwrap_or(false)
}

/// The display clipboard's plain text, or `None` when it holds none.
pub(crate) fn text() -> Option<String> {
    with_clipboard(|clipboard| clipboard.wait_for_text().map(|text| text.to_string())).flatten()
}

/// Publishes `html` (with `plain` as the fallback representation) on the display clipboard.
///
/// HTML has no `gtk_clipboard_set_*` helper: markup travels as an **owner-provided** target, which
/// is the `gtk_clipboard_set_with_data` shape. The callback below is what the display calls when
/// another application asks for the selection, and it must answer for the target it was asked
/// about — the `info` field of the [`gtk::TargetEntry`] is how the request says which one.
///
/// # The two channels, and what "success" means
///
/// `text/html` and a plain-text representation are published together, and the write is reported
/// as successful when **either** landed — the same rule the macOS (`setString:forType:` twice) and
/// iOS backends apply. A receiver without markup support reads the plain channel, and that is a
/// real delivery of what the caller asked for.
///
/// # The verified boundary
///
/// The plain channel is verified end-to-end (see the module tests and `status.md`); the **markup**
/// channel is not. On the one host where GDK could be run against a live display during this
/// round (GTK 3 on macOS, whose quartz backend maps selections onto `NSPasteboard`), a
/// `SelectionData::set` for a non-text target came back **empty** on the reading side: GDK's
/// quartz backend serves only UTF8_STRING from owner-provided data. `SelectionData::set` with
/// format 8 and the requested target is the documented mechanism on X11 and XWayland, which is
/// what this backend targets, but this crate has no X11 host in its gate suite, so the markup arm
/// is reported here as *unverified on this host* rather than claimed.
pub(crate) fn set_html(html: &str, plain: &str) -> bool {
    let html_owned: String = html.into();
    let plain_owned: String = plain.into();
    let owned = with_clipboard(|clipboard| {
        // The atom is interned **inside** the guard, not before it: `gdk_atom_intern` asserts
        // main-thread initialisation, so interning first turned a worker-thread call into a panic
        // instead of the `false` the guard exists to return.
        let html_atom = gtk::gdk::Atom::intern(HTML);
        let targets = [
            gtk::TargetEntry::new(HTML, gtk::TargetFlags::empty(), HTML_INFO),
            gtk::TargetEntry::new("UTF8_STRING", gtk::TargetFlags::empty(), PLAIN_INFO),
            gtk::TargetEntry::new(PLAIN_TEXT, gtk::TargetFlags::empty(), PLAIN_INFO),
        ];
        let offered = clipboard.set_with_data(&targets, move |_clipboard, selection, info| {
            // `selection.set_text` claims UTF8_STRING/TEXT; the HTML arm has no helper, so the
            // bytes go through the generic `set` with format 8 (`guchar`, the only format GDK
            // supports for `GtkSelectionData`).
            match info {
                HTML_INFO => selection.set(&html_atom, 8, html_owned.as_bytes()),
                PLAIN_INFO => {
                    selection.set_text(&plain_owned);
                }
                _ => {}
            }
        });
        if offered {
            // `store()` hands the selection to the clipboard manager so the value survives this
            // process; without it the payload disappears on exit.
            clipboard.store();
        }
        offered
    })
    .unwrap_or(false);

    if !owned {
        return false;
    }
    // The read-back is the evidence: `set_with_data` only says the selection was offered.
    let markup = self::html().is_some_and(|stored| stored == html);
    if !markup {
        log::warn!(
            "[linux clipboard] the text/html target did not come back from the display; the \
             plain-text channel is what a receiver will read"
        );
    }
    markup || text().is_some_and(|stored| stored == plain)
}

/// The display clipboard's HTML, or `None` when it holds none.
pub(crate) fn html() -> Option<String> {
    with_clipboard(|clipboard| {
        // Interned inside the guard for the reason given in `set_html`.
        let selection = clipboard.wait_for_contents(&gtk::gdk::Atom::intern(HTML))?;
        let bytes = selection.data();
        if bytes.is_empty() {
            return None;
        }
        String::from_utf8(bytes).ok()
    })
    .flatten()
}

/// Whether the display clipboard holds data of `content_type` (a MIME type).
pub(crate) fn has_format(content_type: &str) -> bool {
    match content_type {
        PLAIN_TEXT => text().is_some(),
        HTML => html().is_some(),
        // Images, RTF and file lists have no representation here; see the backend's docs.
        _ => false,
    }
}

/// The Linux/GTK host clipboard.
///
/// # What it carries, and what it does not
///
/// Text and HTML — the pair the macOS, iOS, HarmonyOS and Android backends implement. The
/// remaining kinds are refused (`false` / `None`) rather than silently dropped:
///
/// * an image would have to become a `gdk_pixbuf::Pixbuf`, and the RGBA buffer this crate carries
///   is `&[u8]` with no guaranteed lifetime for a zero-copy wrapper — a copy would double the
///   allocation for a payload whose only consumer is another application;
/// * RTF has no GDK target of its own and no verifier in this crate's gate suite;
/// * a file list would have to become `text/uri-list`, and a URI list is a set of *URIs*, not
///   paths, so a `PathBuf` cannot be published without a claim about how it should be encoded.
///
/// # State
///
/// None. GDK is the state: the clipboard is looked up per call, which is also what keeps this type
/// `Send + Sync` (the trait requires it) even though `gtk::Clipboard` is neither.
pub struct GdkClipboard;

impl GdkClipboard {
    /// Creates the backend.
    pub fn new() -> Self {
        Self
    }
}

impl Default for GdkClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl RichClipboardBackend for GdkClipboard {
    fn set_contents(&self, content: ClipboardContent) -> bool {
        match content {
            ClipboardContent::Text(text) => set_text(&text),
            ClipboardContent::Html { html, plain } => set_html(&html, &plain),
            other => {
                log::warn!(
                    "[linux clipboard] {} is not carried by this backend; the write is reported \
                     as refused rather than pretended",
                    other.content_type()
                );
                false
            }
        }
    }

    fn get_contents(&self) -> Option<ClipboardContent> {
        if let Some(html) = html() {
            return Some(ClipboardContent::Html { html, plain: text().unwrap_or_default() });
        }
        text().map(ClipboardContent::Text)
    }

    fn has_format(&self, content_type: &str) -> bool {
        has_format(content_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A thread that never called `gtk::init` must be answered `None`/`false`, not crashed into.
    ///
    /// This is the guard every entry point opens with, and the one that keeps a worker thread from
    /// touching GTK objects it does not own. It holds on any host: a non-main thread is a
    /// non-main thread whether or not a display exists.
    #[test]
    fn a_non_main_thread_is_answered_instead_of_touching_gtk() {
        let answer = std::thread::spawn(|| (text(), html(), has_format(PLAIN_TEXT), set_text("x")))
            .join()
            .expect("the probe thread must not panic");
        assert_eq!(answer, (None, None, false, false));
    }
}
