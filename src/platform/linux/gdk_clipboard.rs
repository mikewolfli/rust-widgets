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
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// How long a clipboard read may wait for the selection's owner.
///
/// GDK's `wait_for_text`/`wait_for_contents` run a nested main loop with no bound: a wedged
/// selection owner freezes the caller for as long as it stays wedged (G-602). A paste is a user
/// action, so this backend waits the same short budget the Wayland backend uses and then answers
/// honestly rather than blocking the UI thread.
const CLIPBOARD_WAIT: Duration = Duration::from_millis(16);

/// Runs the default GLib context until `slot` is filled or [`CLIPBOARD_WAIT`] expires.
///
/// This is the bounded form of the nested loop `wait_for_*` runs: the request is queued with
/// `request_*`, and only the events that can carry its answer are pumped here. A callback that
/// arrives after the deadline still runs later against its own `Rc`, so nothing is left dangling;
/// the answer is simply not observed by this call.
fn wait_for<T>(slot: &Rc<RefCell<Option<T>>>) -> Option<T> {
    let context = gtk::glib::MainContext::default();
    let deadline = Instant::now() + CLIPBOARD_WAIT;
    loop {
        if let Some(value) = slot.borrow_mut().take() {
            return Some(value);
        }
        if Instant::now() >= deadline {
            return None;
        }
        while context.pending() {
            let _ = context.iteration(false);
            if slot.borrow().is_some() {
                break;
            }
        }
        if slot.borrow().is_some() {
            continue;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

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

/// A revision bumped by every `owner-change` signal the display clipboard emits.
///
/// # Why a counter rather than a callback (P-202)
///
/// [`Platform::clipboard_generation`](crate::platform::Platform::clipboard_generation) is a
/// polled revision by contract: a clipboard-manager host compares consecutive readings and
/// re-reads the clipboard when they differ, because the library cannot push an event onto a host
/// loop it does not own. GDK publishes exactly the underlying fact — `owner-change` fires whenever
/// the selection's owner changes, including when this process takes it by writing — so the
/// counter is a real measurement, not a derived guess.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The clipboard object the `owner-change` handler is attached to, as a raw pointer.
///
/// A display swap (an X session replacing its display) hands out a new `GtkClipboard`, and a
/// handler left on the old object would never fire again. Comparing the pointer re-arms the watch
/// for the new object and leaves at most one stale handler on the dead one, which is harmless.
static WATCHED_CLIPBOARD: AtomicUsize = AtomicUsize::new(0);

/// Attaches the `owner-change` handler to `clipboard`, once per clipboard object.
fn watch_owner_change(clipboard: &gtk::Clipboard) {
    use gtk::glib::prelude::{ObjectExt as _, ObjectType as _};
    let pointer = clipboard.as_ptr() as usize;
    if WATCHED_CLIPBOARD.swap(pointer, Ordering::SeqCst) == pointer {
        return;
    }
    // The signal's payload is a `GdkEventOwnerChange`, which gtk-rs 0.18 does not bind as a typed
    // signal; `connect_local` takes the marshalled `Value` list instead, and this handler never
    // reads it. `connect_local` rather than `connect`: GTK emits the signal on the main thread,
    // which is the thread this is attached from, and it must not require the handler to be
    // `Send + Sync` for a `!Send` toolkit object.
    clipboard.connect_local("owner-change", false, |_| {
        GENERATION.fetch_add(1, Ordering::SeqCst);
        None
    });
    // A live handler means this backend can report changes, so the counter starts at 1: the trait
    // reserves `0` for "not reported", and "watch installed, nothing changed yet" is a different
    // fact from "this backend cannot tell".
    GENERATION.fetch_max(1, Ordering::SeqCst);
}

/// A revision that changes whenever the display clipboard's owner changes.
///
/// `0` until a handler could be attached — no GTK main thread, no display, or no clipboard — which
/// is the trait's honest "this backend cannot report it". After that it is always non-zero, so a
/// host that saw a reading can compare it against the next one.
pub(crate) fn generation() -> u64 {
    if GENERATION.load(Ordering::SeqCst) == 0 {
        // First read from the GTK main thread installs the watch. A read from any other thread
        // cannot touch GTK and simply observes whatever the main thread installed (P-202).
        let _ = with_clipboard(watch_owner_change);
    }
    GENERATION.load(Ordering::SeqCst)
}

/// Puts `text` on the display clipboard as plain text.
///
/// `gtk_clipboard_set_text` returns nothing, so the write is reported as **accepted**: GDK holds
/// the selection after the call, and `store()` hands it to the clipboard manager. The former
/// confirmation was a synchronous `wait_for_text()`, a nested main loop with no bound that could
/// freeze the caller on a wedged selection owner (G-602); that read-back is gone. A caller that
/// wants the value reads it through [`text`], which waits at most [`CLIPBOARD_WAIT`].
///
/// # Empty string
///
/// GDK has no "empty string" selection: `set_text("")` leaves the clipboard holding **no text**,
/// so a later [`text`] answers `None` — the same answer `clear()` gives. The write still reports
/// `true` because the selection was offered; the value being empty is not a failure.
pub(crate) fn set_text(text: &str) -> bool {
    with_clipboard(|clipboard| {
        clipboard.set_text(text);
        // `store()` asks the clipboard manager to keep the value after this process exits.
        // Without it the text vanishes on exit, which is not what a user who pressed Ctrl+C
        // expects — and on a session with no manager it is a no-op, not a failure.
        clipboard.store();
        // # Why the write is no longer confirmed by a read-back
        //
        // `gtk_clipboard_set_text` returns nothing, so the old evidence was a synchronous
        // `wait_for_text()` — a nested main loop that can wait forever on a wedged selection
        // owner (G-602). The write is now reported as accepted (the selection was offered) and
        // the caller can read it back through [`text`], which is bounded. Reporting `false` on a
        // timeout would claim a failure that did not happen; reporting `true` for an offered
        // selection is the same contract the Wayland backend publishes.
        true
    })
    .unwrap_or(false)
}

/// The display clipboard's plain text, or `None` when it holds none.
pub(crate) fn text() -> Option<String> {
    with_clipboard(|clipboard| {
        let slot = Rc::new(RefCell::new(None));
        let answer = Rc::clone(&slot);
        clipboard.request_text(move |_clipboard, text| {
            *answer.borrow_mut() = Some(text.map(|text| text.to_string()));
        });
        wait_for(&slot).flatten()
    })
    .flatten()
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
/// The write is reported by `set_with_data`, which answers whether the selection was offered.
/// The old read-back that tried to prove delivery ran a nested main loop with no bound, so it is
/// gone (G-602); a caller that needs the bytes reads them through the bounded [`html`].
///
/// The **markup** channel's delivery is still unverified on a live X11 host: on the one host where
/// GDK could be run against a live display during an earlier round (GTK 3 on macOS, whose quartz
/// backend maps selections onto `NSPasteboard`), a `SelectionData::set` for a non-text target came
/// back empty on the reading side, because that backend serves only UTF8_STRING from
/// owner-provided data. `SelectionData::set` with format 8 and the requested target is the
/// documented mechanism on X11 and XWayland, which is what this backend targets.
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

    // `set_with_data` reports whether the selection was offered; the read-back that used to
    // confirm it ran a nested main loop with no bound (G-602), so the offered selection is the
    // answer here and a caller that needs the bytes reads them through the bounded [`html`].
    owned
}

/// The display clipboard's HTML, or `None` when it holds none.
pub(crate) fn html() -> Option<String> {
    with_clipboard(|clipboard| {
        // Interned inside the guard for the reason given in `set_html`.
        let atom = gtk::gdk::Atom::intern(HTML);
        let slot = Rc::new(RefCell::new(None));
        let answer = Rc::clone(&slot);
        clipboard.request_contents(&atom, move |_clipboard, selection| {
            let bytes = selection.data();
            *answer.borrow_mut() =
                Some(if bytes.is_empty() { None } else { String::from_utf8(bytes).ok() });
        });
        wait_for(&slot).flatten()
    })
    .flatten()
}

/// Whether the display clipboard holds data of `content_type` (a MIME type).
pub(crate) fn has_format(content_type: &str) -> bool {
    match content_type {
        PLAIN_TEXT => text().is_some(),
        HTML => html().is_some(),
        // Every other MIME type — an application-defined one, or a fixed kind this backend does
        // not carry (`image/png`, `text/rtf`, `text/uri-list`) — is answered by asking the
        // selection for that target directly. This is what makes `has_format` truthful for a
        // custom type instead of only for the two the backend writes itself.
        other => holds_target(other),
    }
}

/// Whether the display clipboard offers `content_type` as a selection target.
///
/// The read is the evidence: `wait_for_contents` returns `Some` exactly when the requested target
/// is offered, so asking for the contents with an empty payload still answers "the target is
/// there" rather than "the payload is non-empty".
fn holds_target(content_type: &str) -> bool {
    with_clipboard(|clipboard| {
        let atom = gtk::gdk::Atom::intern(content_type);
        // The callback firing is the evidence that the target is offered; the payload itself is
        // irrelevant to the question and reading it could block on a large transfer.
        let slot = Rc::new(RefCell::new(None));
        let answer = Rc::clone(&slot);
        clipboard.request_contents(&atom, move |_clipboard, _selection| {
            *answer.borrow_mut() = Some(true);
        });
        wait_for(&slot).unwrap_or(false)
    })
    .unwrap_or(false)
}

/// Publishes `data` under the application-defined `content_type` and reports whether it was stored.
///
/// This is the generic form of [`set_html`]: the type is interned as its own selection target and
/// served from an owner callback, which is the documented X11/XWayland mechanism for any MIME type.
/// `set_with_data` reports whether the selection was offered; the former read-back ran an
/// unbounded nested loop and is gone (G-602). A caller that needs the bytes reads them through
/// the bounded [`typed`].
///
/// # The verified boundary
///
/// The same one `set_html`'s markup arm records: the owner-provided-target mechanism is what X11
/// and XWayland specify, and this crate has no X11 host in its gate suite, so a custom target is
/// reported as *verified where a display serves it* (the `MockClipboard` round-trip) and otherwise
/// as *unverified on this host* rather than claimed.
pub(crate) fn set_typed(content_type: &str, data: &[u8]) -> bool {
    let data_owned = data.to_vec();
    let offered = with_clipboard(|clipboard| {
        // Interned inside the guard for the reason `set_html` records: `gdk_atom_intern` asserts
        // main-thread initialisation, so interning before the guard turns a worker-thread call
        // into a panic instead of the `false` the guard exists to return.
        let atom = gtk::gdk::Atom::intern(content_type);
        let targets = [gtk::TargetEntry::new(content_type, gtk::TargetFlags::empty(), 1)];
        let offered = clipboard.set_with_data(&targets, move |_clipboard, selection, _info| {
            selection.set(&atom, 8, &data_owned);
        });
        if offered {
            // `store()` hands the selection to the clipboard manager so the payload survives this
            // process; on a session with no manager it is a no-op, not a failure.
            clipboard.store();
        }
        offered
    })
    .unwrap_or(false);

    // See `set_html`: the offered selection is the answer, and the former read-back ran an
    // unbounded nested loop (G-602).
    offered
}

/// The bytes the display clipboard holds under `content_type`, or `None` when it holds none.
pub(crate) fn typed(content_type: &str) -> Option<Vec<u8>> {
    with_clipboard(|clipboard| {
        // Interned inside the guard for the reason `set_html` records.
        let atom = gtk::gdk::Atom::intern(content_type);
        let slot = Rc::new(RefCell::new(None));
        let answer = Rc::clone(&slot);
        clipboard.request_contents(&atom, move |_clipboard, selection| {
            *answer.borrow_mut() = Some(selection.data());
        });
        wait_for(&slot)
    })
    .flatten()
}

/// The Linux/GTK host clipboard.
///
/// # What it carries, and what it does not
///
/// Text, HTML, and any application-defined MIME type (an X11/Wayland selection target). Text and
/// HTML have dedicated helpers above; a custom type goes through the generic owner-provided-target
/// path in [`set_typed`]/[`typed`], which is the same mechanism the HTML markup arm uses.
///
/// The remaining *structured* kinds are refused (`false` / `None`) rather than silently dropped:
///
/// * an image would have to become a `gdk_pixbuf::Pixbuf`, and the RGBA buffer this crate carries
///   is `&[u8]` with no guaranteed lifetime for a zero-copy wrapper — a copy would double the
///   allocation for a payload whose only consumer is another application;
/// * RTF has no GDK target of its own and no verifier in this crate's gate suite;
/// * a file list would have to become `text/uri-list`, and a URI list is a set of *URIs*, not
///   paths, so a `PathBuf` cannot be published without a claim about how it should be encoded.
///
/// An application that wants to move any of those payloads by name can still do so through
/// [`RichClipboardBackend::set_contents_with_type`], which publishes the bytes under its own type.
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
        match &content {
            ClipboardContent::Text(text) => set_text(text),
            ClipboardContent::Html { html, plain } => set_html(html, plain),
            // An application-defined type is published as its own selection target (see
            // `set_typed`); the structured `Custom` variant and the byte-level
            // `set_contents_with_type` entry therefore take the same path.
            ClipboardContent::Custom { content_type, data } => set_typed(content_type, data),
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

    /// Reads by type directly from the display, rather than from [`Self::get_contents`].
    ///
    /// The structured accessor can only recognize the fixed kinds it knows; a custom type has to be
    /// asked for by name, which is what this native typed channel does. The fixed byte-oriented
    /// kinds are served too, so a caller that reads everything by type gets a uniform answer.
    fn get_contents_for_type(&self, content_type: &str) -> Option<Vec<u8>> {
        match content_type {
            PLAIN_TEXT => text().map(String::into_bytes),
            HTML => html().map(String::into_bytes),
            other => typed(other),
        }
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
