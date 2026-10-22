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
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use wayland_client::protocol::wl_data_device::WlDataDevice;
use wayland_client::protocol::wl_data_device_manager::WlDataDeviceManager;
use wayland_client::protocol::wl_data_offer::WlDataOffer;
use wayland_client::protocol::wl_data_source::WlDataSource;
use wayland_client::{Connection, QueueHandle};

/// The text MIME types, most specific first.
const TEXT_MIMES: [&str; 4] = ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "STRING"];
/// How long a read waits for the source application before answering the local copy.
const READ_TIMEOUT_MS: i32 = 500;
/// How long the caller waits for a typed clipboard read before giving up for this call.
///
/// The actual pipe read still uses [`READ_TIMEOUT_MS`] on a worker thread; this is the UI-facing
/// bound. A source app that is slow to serve the MIME type must not freeze the frame loop.
const TYPED_READ_WAIT_MS: u64 = 16;

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
    /// The last successful remote read.
    ///
    /// A read no longer blocks the caller: the `receive` request is queued on the caller's
    /// thread (proxies are thread-affine) and the pipe is drained on a worker. This is the value
    /// that worker publishes, so a second read observes the result without another transfer.
    remote: Option<ClipboardContent>,
    /// Set while a worker is draining a receive pipe, so repeated reads do not start a thread
    /// each and flood the source application with requests.
    fetching: bool,
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
                remote: None,
                fetching: false,
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
        inner.remote = None;
        inner.fetching = false;
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
        inner.remote = None;
        inner.fetching = false;
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
        // A new offer means the cached remote content describes the previous selection.
        inner.remote = None;
        inner.fetching = false;
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
        // The new selection invalidates the cached read from the old one.
        inner.remote = None;
        inner.fetching = false;
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
            // The first read queues a worker that drains the source pipe. Returning `None`
            // immediately made the **first** paste after an external copy look empty even though
            // the data was on its way. Paste is a user action, not a frame path, so wait for the
            // worker for a bounded interval (the same 16 ms budget as the typed read) and then
            // answer honestly.
            for _ in 0..16 {
                if let Some(content) = self.read_contents() {
                    return Some(match content {
                        ClipboardContent::Text(text) => text,
                        ClipboardContent::Html { plain, html } => {
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
                if !lock(&self.inner).fetching {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            return Some(String::new());
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
    /// Returns the cached remote read immediately and, when there is no cache, queues the
    /// `receive` requests and drains their pipes on a worker thread. The caller therefore never
    /// waits for the source application: a slow or wedged source can no longer freeze the UI for
    /// `READ_TIMEOUT_MS`.
    fn read_contents(&self) -> Option<ClipboardContent> {
        let (offer, mimes, conn, cached, fetching, local) = {
            let inner = lock(&self.inner);
            (
                inner.offer.as_ref().map(|offer| offer.proxy.clone()),
                inner.offer.as_ref().map(|offer| offer.mimes.clone()).unwrap_or_default(),
                inner.channel.as_ref().map(|channel| channel.conn.clone()),
                inner.remote.clone(),
                inner.fetching,
                inner.local.clone(),
            )
        };
        let (Some(offer), Some(conn)) = (offer, conn) else {
            // No live selection: the locally written content is the only answer.
            return local;
        };
        if cached.is_none() && !fetching {
            // Queue the receives on *this* thread: `WlDataOffer` is a protocol proxy bound to the
            // session's connection. Only the pipe descriptors cross to the worker.
            let plan = read_plan(&mimes);
            let mut requests: Vec<(String, OwnedFd)> = Vec::with_capacity(plan.len());
            for mime in plan {
                if let Some(read_end) = receive_to_pipe(&offer, &conn, mime) {
                    requests.push((mime.to_string(), read_end));
                }
            }
            if !requests.is_empty() {
                lock(&self.inner).fetching = true;
                let inner = self.inner.clone();
                let spawned = std::thread::Builder::new()
                    .name("wayland-clipboard-read".to_string())
                    .spawn(move || {
                        let reads: Vec<(String, Vec<u8>)> = requests
                            .into_iter()
                            .filter_map(|(mime, read_end)| {
                                finish_read(read_end).map(|bytes| (mime, bytes))
                            })
                            .collect();
                        let content = content_from_reads(reads);
                        let mut guard = lock(&inner);
                        guard.fetching = false;
                        if let Some(content) = content {
                            guard.remote = Some(content);
                            guard.generation = guard.generation.wrapping_add(1);
                        }
                        drop(guard);
                        // A host that renders clipboard state can now observe the result.
                        crate::platform::frame_wait::wake();
                    });
                if spawned.is_err() {
                    lock(&self.inner).fetching = false;
                }
            }
        }
        cached
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
        inner.remote = None;
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
                // The pipe read can wait up to `READ_TIMEOUT_MS` for the source app. Run it on
                // a short-lived worker and bound the caller's wait, so a wedged source cannot
                // stall the UI thread; a timeout is reported as "no typed payload" and the
                // local copy below still answers.
                let mime = content_type.to_string();
                let worker_offer = offer.clone();
                let worker_conn = conn.clone();
                let (sender, receiver) = std::sync::mpsc::sync_channel(1);
                let spawned = std::thread::Builder::new()
                    .name("rw-clipboard-typed".to_string())
                    .spawn(move || {
                        let _ = sender.send(read_mime(&worker_offer, &worker_conn, &mime));
                    });
                let read = match spawned {
                    Ok(_) => receiver
                        .recv_timeout(std::time::Duration::from_millis(TYPED_READ_WAIT_MS))
                        .ok()
                        .flatten(),
                    // Thread creation can fail only on resource exhaustion; the synchronous
                    // read is then the honest fallback rather than losing the paste.
                    Err(error) => {
                        log::debug!(
                            "[wayland] typed clipboard worker could not start ({error}); reading inline"
                        );
                        read_mime(&offer, &conn, content_type)
                    }
                };
                if let Some(bytes) = read {
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

/// The MIME types to request for a plain/rich paste, in priority order.
///
/// One decision shared by the caller and the worker: the caller needs it to queue the
/// `receive` requests, and the worker needs the same order to interpret the bytes.
fn read_plan(mimes: &[String]) -> Vec<&str> {
    let has = |mime: &str| mimes.iter().any(|offered| offered == mime);
    if has("text/html") {
        let mut plan = vec!["text/html"];
        if let Some(plain) = TEXT_MIMES.iter().find(|mime| has(mime)) {
            plan.push(*plain);
        }
        return plan;
    }
    if let Some(text) = TEXT_MIMES.iter().find(|mime| has(mime)) {
        return vec![*text];
    }
    // A single non-text type (an application-defined type) round-trips by name.
    mimes
        .iter()
        .find(|mime| !TEXT_MIMES.contains(&mime.as_str()) && mime.as_str() != "text/html")
        .map(|mime| vec![mime.as_str()])
        .unwrap_or_default()
}

/// Queues `receive` for `mime` and returns the pipe's read end.
///
/// Only this half runs on the caller's thread: `WlDataOffer` is a proxy bound to the session's
/// connection, while the descriptor is free to cross to a worker. The wait for the source
/// application happens in [`finish_read`], not here.
fn receive_to_pipe(offer: &WlDataOffer, conn: &Connection, mime: &str) -> Option<OwnedFd> {
    let mut fds = [0i32; 2];
    // SAFETY: `fds` is a valid two-element array; `pipe2` either fills both entries or fails
    // without changing them, and the owned ends below are only constructed on success.
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if rc != 0 {
        log::warn!("[wayland] clipboard: pipe2 failed");
        return None;
    }
    // SAFETY: `fds[1]` is a fresh descriptor this function owns and has not wrapped elsewhere.
    let write_end = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    offer.receive(mime.to_string(), write_end.as_fd());
    // Dropping our write end is what lets the receiver observe EOF after the compositor's copy.
    drop(write_end);
    if let Err(e) = conn.flush() {
        log::warn!("[wayland] clipboard: receive flush failed: {e}");
        // SAFETY: `fds[0]` is fresh and owned here; closing it once is required.
        unsafe { libc::close(fds[0]) };
        return None;
    }
    // SAFETY: `fds[0]` is a fresh descriptor this function owns and has not wrapped elsewhere.
    Some(unsafe { OwnedFd::from_raw_fd(fds[0]) })
}

/// Waits for the source application to write and drains the pipe.
///
/// FFI/safety: `read_end` is owned by this call, so it is closed exactly once — by the `File`
/// wrapper on the read path, or by dropping the `OwnedFd` on the timeout path.
fn finish_read(read_end: OwnedFd) -> Option<Vec<u8>> {
    let fd = read_end.as_raw_fd();
    let mut pollfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
    // SAFETY: `pollfd` is a valid array of one entry for the duration of the call.
    let ready = unsafe { libc::poll(&mut pollfd, 1, READ_TIMEOUT_MS) };
    if ready <= 0 {
        log::warn!("[wayland] clipboard: transfer timed out or failed");
        return None;
    }
    let mut file = std::fs::File::from(read_end);
    let mut bytes = Vec::new();
    use std::io::Read;
    match file.read_to_end(&mut bytes) {
        Ok(_) => Some(bytes),
        Err(e) => {
            log::warn!("[wayland] clipboard: reading the transfer failed: {e}");
            None
        }
    }
}

/// Turns the completed reads into the rich-content shape the read plan asked for.
fn content_from_reads(mut reads: Vec<(String, Vec<u8>)>) -> Option<ClipboardContent> {
    if let Some(position) = reads.iter().position(|(mime, _)| mime == "text/html") {
        let (_, html_bytes) = reads.remove(position);
        let html = decode(html_bytes)?;
        let plain = reads
            .iter()
            .find(|(mime, _)| is_text_mime(mime))
            .and_then(|(_, bytes)| decode(bytes.clone()))
            .unwrap_or_default();
        return Some(ClipboardContent::Html { html, plain });
    }
    if let Some((_, bytes)) = reads.iter().find(|(mime, _)| is_text_mime(mime)) {
        return decode(bytes.clone()).map(ClipboardContent::Text);
    }
    let (content_type, data) = reads.into_iter().next()?;
    Some(ClipboardContent::Custom { content_type, data })
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
        let content = ClipboardContent::Html { html: "<b>x</b>".into(), plain: "x".into() };
        let mimes = offered_mimes(&content);
        assert_eq!(mimes, vec!["text/html", "text/plain;charset=utf-8", "text/plain"]);
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
