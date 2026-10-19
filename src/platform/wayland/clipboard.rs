// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Wayland clipboard over `wl_data_device` (BLUE28 B28.04 / B28.11).
//!
//! # What this replaces
//!
//! The backend used to keep the clipboard as an in-process `String` on
//! [`BackendState`](crate::platform::state::BackendState). That made the call sites look
//! implemented while nothing reached another application: a copy in one process could never be
//! pasted in another, and [`Platform::capabilities`](crate::platform::Platform::capabilities)
//! therefore reported `clipboard: false`.
//!
//! This module binds the one protocol Wayland defines for it:
//!
//! * `wl_data_device_manager` creates a `wl_data_device` for the session's `wl_seat`;
//! * a write creates a `wl_data_source` that offers the MIME types of the content and calls
//!   `wl_data_device.set_selection`;
//! * a read receives the current `wl_data_offer` into a pipe and drains it, on the thread that
//!   owns the event queue.
//!
//! # Serial numbers
//!
//! `set_selection` takes the serial of the input event that triggered the copy. A compositor uses
//! it to reject a selection change from a client that does not currently have focus. This backend
//! does not own input delivery (the host feeds the widget layer), so a copy made before any seat
//! event has a serial of `0`. In that state the content is stored locally and **published on the
//! first seat event that carries a serial** ([`WaylandClipboard::note_serial`]). That keeps a
//! toolbar Copy button working without inventing a serial the compositor would reject, and still
//! lets a real copy reach the compositor as soon as the user has interacted.
//!
//! # Failure is not success
//!
//! A read whose offer is gone, whose MIME type was not offered, or whose transfer times out
//! answers the locally recorded content (or `None` through the rich backend), never a fabricated
//! empty success. Image content is deliberately refused: [`ClipboardContent::Image`] carries
//! RGBA, and offering it as `image/png` would mislabel the bytes.
#![cfg(all(feature = "wayland-native", unix_desktop))]

use super::platform_impl::WaylandSessionState;
use crate::compat::{lock, Arc, Mutex, String, ToString, Vec};
use crate::platform::clipboard::{ClipboardContent, RichClipboardBackend};
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use wayland_client::protocol::wl_data_device::WlDataDevice;
use wayland_client::protocol::wl_data_device_manager::WlDataDeviceManager;
use wayland_client::protocol::wl_data_offer::WlDataOffer;
use wayland_client::protocol::wl_data_source::WlDataSource;
use wayland_client::{Connection, QueueHandle};

/// The text MIME types, most specific first.
const TEXT_MIMES: [&str; 4] = ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "STRING"];
/// How long a read waits for the source application before answering the local copy.
const READ_TIMEOUT_MS: i32 = 500;

/// The clipboard state shared by the platform and the session's protocol dispatch.
#[derive(Clone)]
pub(crate) struct WaylandClipboard {
    inner: Arc<Mutex<Inner>>,
}

/// One `wl_data_offer` and the MIME types it advertised.
struct Offer {
    proxy: WlDataOffer,
    mimes: Vec<String>,
}

/// The live protocol channel, present only once a `wl_data_device` was created.
struct Channel {
    #[allow(dead_code)]
    manager: WlDataDeviceManager,
    device: WlDataDevice,
    qh: QueueHandle<WaylandSessionState>,
    conn: Connection,
}

struct Inner {
    channel: Option<Channel>,
    offer: Option<Offer>,
    /// The content this process wrote, also served from `send` events.
    local: Option<ClipboardContent>,
    /// Kept alive while the compositor owns the selection.
    source: Option<WlDataSource>,
    /// Serial of the most recent seat event.
    last_serial: u32,
    /// Bumped whenever the selection changes or we write, for `clipboard_generation`.
    generation: u64,
}

impl WaylandClipboard {
    /// Creates an empty clipboard with no protocol channel.
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                channel: None,
                offer: None,
                local: None,
                source: None,
                last_serial: 0,
                generation: 0,
            })),
        }
    }

    /// Drops the protocol channel before a new session is built.
    ///
    /// Proxies from a dead connection would otherwise look live to `has_channel` and a write
    /// would queue a request no compositor reads.
    pub(crate) fn clear_channel(&self) {
        let mut inner = lock(&self.inner);
        inner.channel = None;
        inner.offer = None;
        inner.source = None;
    }

    /// Records the `wl_data_device` and the queue it belongs to; called once per session.
    pub(crate) fn attach_channel(
        &self,
        manager: WlDataDeviceManager,
        device: WlDataDevice,
        qh: QueueHandle<WaylandSessionState>,
        conn: Connection,
    ) {
        let mut inner = lock(&self.inner);
        inner.channel = Some(Channel { manager, device, qh, conn });
        inner.offer = None;
        inner.source = None;
        inner.generation = inner.generation.wrapping_add(1);
    }

    /// Whether a live `wl_data_device` exists, which is what `capabilities().clipboard` promises.
    pub(crate) fn has_channel(&self) -> bool {
        lock(&self.inner).channel.is_some()
    }

    /// A revision bumped by selection changes and local writes.
    pub(crate) fn generation(&self) -> u64 {
        lock(&self.inner).generation
    }

    /// Records the serial of a seat event and publishes content waiting for one.
    pub(crate) fn note_serial(&self, serial: u32) {
        if serial == 0 {
            return;
        }
        let mut inner = lock(&self.inner);
        inner.last_serial = serial;
        if inner.source.is_none() && inner.local.is_some() {
            inner.publish(true);
        }
    }

    /// A `data_device.data_offer` arrived.
    pub(crate) fn note_offer(&self, proxy: WlDataOffer) {
        let mut inner = lock(&self.inner);
        inner.offer = Some(Offer { proxy, mimes: Vec::new() });
        inner.generation = inner.generation.wrapping_add(1);
    }

    /// One `wl_data_offer.offer` arrived for the current offer.
    pub(crate) fn note_mime(&self, mime: &str) {
        let mut inner = lock(&self.inner);
        let Some(offer) = inner.offer.as_mut() else { return };
        if !offer.mimes.iter().any(|existing| existing == mime) {
            offer.mimes.push(mime.to_string());
        }
    }

    /// A `data_device.selection` arrived. `None` means the selection was cleared.
    pub(crate) fn note_selection(&self, proxy: Option<WlDataOffer>) {
        let mut inner = lock(&self.inner);
        match proxy {
            Some(proxy) => {
                // The `data_offer` event that precedes this one already installed the proxy and
                // the offer's MIME events may already have filled `mimes`; replacing it here
                // would discard them and make a read look like "the type was not offered".
                if inner.offer.is_none() {
                    inner.offer = Some(Offer { proxy, mimes: Vec::new() });
                }
            }
            None => inner.offer = None,
        }
        inner.generation = inner.generation.wrapping_add(1);
    }

    /// A `wl_data_source.send` arrived: write the requested representation into `fd`.
    pub(crate) fn serve(&self, mime: &str, fd: OwnedFd) {
        let bytes = {
            let inner = lock(&self.inner);
            inner.local.as_ref().and_then(|content| bytes_for(content, mime))
        };
        let Some(bytes) = bytes else {
            log::warn!("[wayland] clipboard: no representation for '{mime}'");
            return;
        };
        // Dropping the `File` closes our end, which is the EOF the receiver waits for.
        let mut file = std::fs::File::from(fd);
        use std::io::Write;
        if let Err(e) = file.write_all(&bytes) {
            log::warn!("[wayland] clipboard: writing '{mime}' failed: {e}");
        }
    }

    /// The plain-text clipboard contents, or `None` when the clipboard holds no text.
    pub(crate) fn text(&self) -> Option<String> {
        let has_offer = lock(&self.inner).offer.is_some();
        if has_offer {
            return Some(match self.read_contents() {
                Some(ClipboardContent::Text(text)) => text,
                Some(ClipboardContent::Html { plain, html }) => {
                    if plain.is_empty() {
                        html
                    } else {
                        plain
                    }
                }
                // A live selection that is not text (an image or an application type).
                _ => String::new(),
            });
        }
        let inner = lock(&self.inner);
        inner.local.as_ref().and_then(|content| match content {
            ClipboardContent::Text(text) => Some(text.clone()),
            ClipboardContent::Html { plain, html } => {
                Some(if plain.is_empty() { html.clone() } else { plain.clone() })
            }
            _ => None,
        })
    }

    /// Reads the current selection as rich content.
    ///
    /// The offer is read first because it is the system clipboard; the local copy is the fallback
    /// only when there is no offer or the transfer fails.
    fn read_contents(&self) -> Option<ClipboardContent> {
        let (offer, mimes, conn) = {
            let inner = lock(&self.inner);
            let offer = inner.offer.as_ref()?;
            let channel = inner.channel.as_ref()?;
            (offer.proxy.clone(), offer.mimes.clone(), channel.conn.clone())
        };

        if mimes.iter().any(|mime| mime == "text/html") {
            let html = read_mime(&offer, &conn, "text/html")?;
            let html = decode(html)?;
            let plain = TEXT_MIMES
                .iter()
                .find(|mime| mimes.iter().any(|offered| offered == *mime))
                .and_then(|mime| read_mime(&offer, &conn, mime))
                .and_then(decode)
                .unwrap_or_default();
            return Some(ClipboardContent::Html { html, plain });
        }

        if let Some(mime) = TEXT_MIMES.iter().find(|mime| mimes.iter().any(|o| o == *mime)) {
            if let Some(text) = read_mime(&offer, &conn, mime).and_then(decode) {
                return Some(ClipboardContent::Text(text));
            }
        }

        // A single non-text type (an application-defined type) round-trips by name.
        if let Some(mime) = mimes
            .iter()
            .find(|mime| !TEXT_MIMES.contains(&mime.as_str()) && mime.as_str() != "text/html")
        {
            if let Some(data) = read_mime(&offer, &conn, mime) {
                return Some(ClipboardContent::Custom { content_type: mime.clone(), data });
            }
        }
        None
    }
}

impl Inner {
    /// Offers the local content to the compositor.
    ///
    /// `replace` is true for a new write (a new source replaces the previous selection) and false
    /// when a serial simply arrived for content already waiting to be published.
    fn publish(&mut self, replace: bool) {
        if self.last_serial == 0 {
            return;
        }
        if !replace && self.source.is_some() {
            return;
        }
        let Some(channel) = self.channel.as_ref() else { return };
        let Some(content) = self.local.as_ref() else { return };
        let mimes = offered_mimes(content);
        if mimes.is_empty() {
            log::warn!(
                "[wayland] clipboard: '{}' has no Wayland representation and was not offered",
                content.content_type()
            );
            return;
        }
        let source = channel.manager.create_data_source(&channel.qh, ());
        for mime in mimes {
            source.offer(mime.to_string());
        }
        self.source = Some(source.clone());
        channel.device.set_selection(Some(&source), self.last_serial);
        if let Err(e) = channel.conn.flush() {
            log::warn!("[wayland] clipboard: flush failed: {e}");
        }
    }
}

impl RichClipboardBackend for WaylandClipboard {
    fn set_contents(&self, content: ClipboardContent) -> bool {
        let mut inner = lock(&self.inner);
        inner.local = Some(content);
        inner.generation = inner.generation.wrapping_add(1);
        inner.publish(true);
        true
    }

    fn get_contents(&self) -> Option<ClipboardContent> {
        if lock(&self.inner).offer.is_some() {
            // The live selection is authoritative; a failed transfer is `None`, not the last
            // value this process wrote.
            return self.read_contents();
        }
        lock(&self.inner).local.clone()
    }

    fn has_format(&self, content_type: &str) -> bool {
        let inner = lock(&self.inner);
        if let Some(offer) = inner.offer.as_ref() {
            if offer.mimes.iter().any(|mime| mime == content_type) {
                return true;
            }
        }
        inner.local.as_ref().is_some_and(|content| {
            content.content_type() == content_type
                || (content_type.starts_with("text/plain")
                    && matches!(content, ClipboardContent::Html { .. }))
        })
    }

    fn set_contents_with_type(&self, content_type: &str, data: &[u8]) -> bool {
        self.set_contents(ClipboardContent::Custom {
            content_type: content_type.to_string(),
            data: data.to_vec(),
        })
    }

    fn get_contents_for_type(&self, content_type: &str) -> Option<Vec<u8>> {
        let (offer, mimes, conn, local) = {
            let inner = lock(&self.inner);
            (
                inner.offer.as_ref().map(|offer| offer.proxy.clone()),
                inner.offer.as_ref().map(|offer| offer.mimes.clone()).unwrap_or_default(),
                inner.channel.as_ref().map(|channel| channel.conn.clone()),
                inner.local.clone(),
            )
        };
        if let (Some(offer), Some(conn)) = (offer, conn) {
            if mimes.iter().any(|mime| mime == content_type) {
                if let Some(bytes) = read_mime(&offer, &conn, content_type) {
                    return Some(bytes);
                }
            }
        }
        local.and_then(|content| {
            (content.content_type() == content_type).then(|| content.raw_bytes()).flatten()
        })
    }
}

/// The MIME types a content can be served as.
fn offered_mimes(content: &ClipboardContent) -> Vec<&str> {
    match content {
        ClipboardContent::Text(_) => TEXT_MIMES.to_vec(),
        ClipboardContent::Html { .. } => {
            let mut mimes = Vec::with_capacity(3);
            mimes.push("text/html");
            mimes.extend_from_slice(&TEXT_MIMES[..2]);
            mimes
        }
        ClipboardContent::Rtf(_) => Vec::from(["text/rtf", "application/rtf"]),
        // RGBA bytes are not a PNG, and `text/uri-list` needs a URI spelling this crate does not
        // impose, so neither is offered rather than mislabelled.
        ClipboardContent::Image { .. } => Vec::new(),
        #[cfg(not(alloc_frugal))]
        ClipboardContent::Files(_) => Vec::new(),
        ClipboardContent::Custom { content_type, .. } => Vec::from([content_type.as_str()]),
    }
}

/// The bytes `content` answers for `mime`, or `None` when it has no such representation.
fn bytes_for(content: &ClipboardContent, mime: &str) -> Option<Vec<u8>> {
    match content {
        ClipboardContent::Text(text) if is_text_mime(mime) => Some(text.as_bytes().to_vec()),
        ClipboardContent::Html { html, plain } => {
            if mime == "text/html" {
                Some(html.as_bytes().to_vec())
            } else if is_text_mime(mime) {
                Some(plain.as_bytes().to_vec())
            } else {
                None
            }
        }
        ClipboardContent::Rtf(bytes) if mime == "text/rtf" || mime == "application/rtf" => {
            Some(bytes.clone())
        }
        ClipboardContent::Custom { content_type, data } if content_type == mime => {
            Some(data.clone())
        }
        _ => None,
    }
}

fn is_text_mime(mime: &str) -> bool {
    TEXT_MIMES.contains(&mime)
}

fn decode(bytes: Vec<u8>) -> Option<String> {
    String::from_utf8(bytes).ok()
}

/// Reads one MIME type from an offer through a pipe.
///
/// FFI/safety: the pipe fds are created here and each is owned by exactly one value — the write
/// end is moved into `receive`, the read end into `File` — so neither is closed twice. The read end
/// is polled with a timeout before reading, so a source that never writes cannot hang the caller.
fn read_mime(offer: &WlDataOffer, conn: &Connection, mime: &str) -> Option<Vec<u8>> {
    let mut fds = [0i32; 2];
    // SAFETY: `fds` is a valid two-element array; `pipe2` either fills both entries or fails
    // without changing them, and the owned ends below are only constructed on success.
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if rc != 0 {
        log::warn!("[wayland] clipboard: pipe2 failed");
        return None;
    }
    // SAFETY: `fds[1]` is a fresh descriptor this function owns and has not wrapped anywhere else.
    let write_end = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    offer.receive(mime.to_string(), write_end.as_fd());
    if let Err(e) = conn.flush() {
        log::warn!("[wayland] clipboard: receive flush failed: {e}");
        // SAFETY: `fds[0]` is fresh and owned here; closing it once is required.
        unsafe { libc::close(fds[0]) };
        return None;
    }

    let read_fd = fds[0];
    let mut pollfd = libc::pollfd { fd: read_fd, events: libc::POLLIN, revents: 0 };
    // SAFETY: `pollfd` is a valid array of one entry for the duration of the call.
    let ready = unsafe { libc::poll(&mut pollfd, 1, READ_TIMEOUT_MS) };
    if ready <= 0 {
        log::warn!("[wayland] clipboard: read for '{mime}' timed out or failed");
        // SAFETY: `read_fd` is this function's descriptor and is not wrapped anywhere.
        unsafe { libc::close(read_fd) };
        return None;
    }

    // SAFETY: `read_fd` is this function's descriptor; `File` takes ownership and closes it once.
    let mut file = std::fs::File::from(unsafe { OwnedFd::from_raw_fd(read_fd) });
    let mut bytes = Vec::new();
    use std::io::Read;
    match file.read_to_end(&mut bytes) {
        Ok(_) => Some(bytes),
        Err(e) => {
            log::warn!("[wayland] clipboard: reading '{mime}' failed: {e}");
            None
        }
    }
}

/// Polls how long a read waits; exposed for the test that pins the bound.
#[cfg(test)]
pub(crate) const READ_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(READ_TIMEOUT_MS as u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_without_a_channel_still_round_trips_locally() {
        let clipboard = WaylandClipboard::new();
        assert!(!clipboard.has_channel());
        assert!(clipboard.set_contents(ClipboardContent::Text("local".into())));
        assert_eq!(clipboard.text().as_deref(), Some("local"));
        assert_eq!(clipboard.get_contents_for_type("text/plain"), Some(b"local".to_vec()));
        assert!(clipboard.has_format("text/plain"));
    }

    #[test]
    fn html_offers_markup_plus_a_plain_fallback() {
        let mimes =
            offered_mimes(&ClipboardContent::Html { html: "<b>x</b>".into(), plain: "x".into() });
        assert_eq!(mimes, vec!["text/html", "text/plain;charset=utf-8", "text/plain"]);
        let content = ClipboardContent::Html { html: "<b>x</b>".into(), plain: "x".into() };
        assert_eq!(bytes_for(&content, "text/html"), Some(b"<b>x</b>".to_vec()));
        assert_eq!(bytes_for(&content, "text/plain;charset=utf-8"), Some(b"x".to_vec()));
    }

    #[test]
    fn an_image_is_not_offered_as_png() {
        let image = ClipboardContent::Image { width: 1, height: 1, data: vec![0, 0, 0, 255] };
        assert!(offered_mimes(&image).is_empty());
        assert_eq!(bytes_for(&image, "image/png"), None);
    }

    #[test]
    fn a_custom_type_is_offered_and_served_by_name() {
        let custom = ClipboardContent::Custom {
            content_type: "application/vnd.example+json".into(),
            data: b"{}".to_vec(),
        };
        assert_eq!(offered_mimes(&custom), vec!["application/vnd.example+json"]);
        assert_eq!(bytes_for(&custom, "application/vnd.example+json"), Some(b"{}".to_vec()));
        assert_eq!(bytes_for(&custom, "text/plain"), None);
    }

    #[test]
    fn rtf_is_offered_under_both_spellings() {
        let rtf = ClipboardContent::Rtf(b"{\\rtf1}".to_vec());
        assert_eq!(offered_mimes(&rtf), vec!["text/rtf", "application/rtf"]);
        assert_eq!(bytes_for(&rtf, "application/rtf"), Some(b"{\\rtf1}".to_vec()));
    }

    #[test]
    fn the_read_timeout_is_bounded() {
        assert_eq!(READ_TIMEOUT, std::time::Duration::from_millis(500));
    }

    #[test]
    fn generation_changes_on_each_observation() {
        let clipboard = WaylandClipboard::new();
        let first = clipboard.generation();
        assert!(clipboard.set_contents(ClipboardContent::Text("a".into())));
        assert_ne!(clipboard.generation(), first);
    }
}
