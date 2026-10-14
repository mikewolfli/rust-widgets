// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The OpenHarmony system pasteboard — `OH_Pasteboard` over UDMF.
//!
//! # Why this module exists
//!
//! The clipboard was the one host service this backend reported as absent
//! (`capabilities().clipboard == false`) and served from `BackendState`, an in-process record.
//! That record is not a clipboard: a copy made by this process is invisible to every other
//! application on the device, and a copy made elsewhere is invisible here. OpenHarmony ships a
//! system pasteboard NDK (`libpasteboard.so`, `libudmf.so`, headers under
//! `native/sysroot/usr/include/database/`), so the honest answer is to bind it rather than to
//! keep reporting the absence.
//!
//! # The shape of the API
//!
//! Two libraries take part, and both are required:
//!
//! * `OH_Pasteboard` (the *store*): `Create`/`Destroy`, `SetData`, `GetData`, `HasType`.
//! * UDMF (the *payload*): `OH_UdmfData` holds records, an `OH_UdmfRecord` holds
//!   representations, and a representation is a *UDS* — `OH_UdsPlainText`, `OH_UdsHtml`.
//!
//! So a write is `Uds → Record → Data → Pasteboard` and a read is the reverse. Every pointer
//! `Create` hands out is destroyed on the path out of this module, including the ones that only
//! exist to be read back.
//!
//! # Why the writes are confirmed by a read
//!
//! `OH_Pasteboard_SetData` returns a status code, but the pasteboard is a system service that a
//! sandboxed process may be refused by — a returned `UDMF_E_OK` proves the call was accepted,
//! not that the value is now on the clipboard. Every write here therefore reads the pasteboard
//! back and compares, which is the same rule the macOS (`writeObjects:`), iOS (`setString:`) and
//! Android (`setPrimaryClip`) backends follow.
//!
//! # Hand-written FFI, and why
//!
//! Like [`super::xcomponent`], the declarations below are written out rather than generated: this
//! crate has no `bindgen` dependency, and the contract being bound is a few dozen lines of a
//! header the SDK ships. The declarations are `#[cfg(target_env = "ohos")]`; on a host build the
//! stand-in [`sys`] module below answers "not available" instead, which keeps the surrounding
//! logic compilable (and unit-testable) anywhere while never pretending a host has an
//! OpenHarmony pasteboard.

use crate::compat::String;
use crate::platform::clipboard::{ClipboardContent, RichClipboardBackend};

/// The MIME type `OH_Pasteboard_HasType` is asked about for plain text.
const PLAIN_TEXT_TYPE: &str = "text/plain";
/// The MIME type `OH_Pasteboard_HasType` is asked about for HTML.
const HTML_TYPE: &str = "text/html";

#[cfg(target_env = "ohos")]
mod sys {
    //! The real bindings: `libpasteboard` and `libudmf` from the OpenHarmony sysroot.
    //!
    //! The link directives live in `build.rs` (gated on `target_env == "ohos"`), because the
    //! libraries only exist in the SDK's per-architecture `sysroot/usr/lib` directory, which
    //! `cargo ohos` passes with `-L`.

    use crate::compat::{String, Vec};
    use core::ffi::{c_char, CStr};

    /// The status code UDMF and the pasteboard return on success (`UDMF_E_OK`).
    ///
    /// Declared here rather than at the module root: only this half of the backend has a status
    /// code to compare against, and a file-level constant would warn as dead code on every host
    /// build, where the stand-in module below answers without one.
    const OHOS_OK: i32 = 0;

    /// Opaque `OH_Pasteboard`.
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub(super) struct OhPasteboard {
        _private: [u8; 0],
    }

    /// Opaque `OH_UdmfData`.
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub(super) struct OhUdmfData {
        _private: [u8; 0],
    }

    /// Opaque `OH_UdmfRecord`.
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub(super) struct OhUdmfRecord {
        _private: [u8; 0],
    }

    /// Opaque `OH_UdsPlainText`.
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub(super) struct OhUdsPlainText {
        _private: [u8; 0],
    }

    /// Opaque `OH_UdsHtml`.
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub(super) struct OhUdsHtml {
        _private: [u8; 0],
    }

    extern "C" {
        fn OH_Pasteboard_Create() -> *mut OhPasteboard;
        fn OH_Pasteboard_Destroy(pasteboard: *mut OhPasteboard);
        fn OH_Pasteboard_SetData(pasteboard: *mut OhPasteboard, data: *mut OhUdmfData) -> i32;
        fn OH_Pasteboard_GetData(
            pasteboard: *mut OhPasteboard,
            status: *mut i32,
        ) -> *mut OhUdmfData;
        fn OH_Pasteboard_HasType(pasteboard: *mut OhPasteboard, data_type: *const c_char) -> bool;

        fn OH_UdmfData_Create() -> *mut OhUdmfData;
        fn OH_UdmfData_Destroy(data: *mut OhUdmfData);
        fn OH_UdmfData_AddRecord(data: *mut OhUdmfData, record: *mut OhUdmfRecord) -> i32;
        fn OH_UdmfData_GetPrimaryPlainText(
            data: *mut OhUdmfData,
            plain_text: *mut OhUdsPlainText,
        ) -> i32;
        fn OH_UdmfData_GetRecord(data: *mut OhUdmfData, index: u32) -> *mut OhUdmfRecord;

        fn OH_UdmfRecord_Create() -> *mut OhUdmfRecord;
        fn OH_UdmfRecord_Destroy(record: *mut OhUdmfRecord);
        fn OH_UdmfRecord_AddPlainText(
            record: *mut OhUdmfRecord,
            plain_text: *mut OhUdsPlainText,
        ) -> i32;
        fn OH_UdmfRecord_AddHtml(record: *mut OhUdmfRecord, html: *mut OhUdsHtml) -> i32;
        fn OH_UdmfRecord_GetHtml(record: *mut OhUdmfRecord, html: *mut OhUdsHtml) -> i32;

        fn OH_UdsPlainText_Create() -> *mut OhUdsPlainText;
        fn OH_UdsPlainText_Destroy(plain_text: *mut OhUdsPlainText);
        fn OH_UdsPlainText_SetContent(
            plain_text: *mut OhUdsPlainText,
            content: *const c_char,
        ) -> i32;
        fn OH_UdsPlainText_GetContent(plain_text: *mut OhUdsPlainText) -> *const c_char;

        fn OH_UdsHtml_Create() -> *mut OhUdsHtml;
        fn OH_UdsHtml_Destroy(html: *mut OhUdsHtml);
        fn OH_UdsHtml_SetContent(html: *mut OhUdsHtml, content: *const c_char) -> i32;
        fn OH_UdsHtml_SetPlainContent(html: *mut OhUdsHtml, content: *const c_char) -> i32;
        fn OH_UdsHtml_GetContent(html: *mut OhUdsHtml) -> *const c_char;
    }

    /// Whether this build can reach an OpenHarmony pasteboard. Always `true` here.
    pub(super) fn available() -> bool {
        true
    }

    /// A NUL-terminated copy of `value`.
    ///
    /// Built by hand rather than with `CString` because the `mini` profile compiles this module
    /// under `no_std` with `alloc`, where `std::ffi` does not exist. The buffer outlives every
    /// call that borrows it: it is a local in the function that passes the pointer.
    fn c_string(value: &str) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(value.len() + 1);
        buffer.extend_from_slice(value.as_bytes());
        buffer.push(0);
        buffer
    }

    /// Reads a borrowed C string, or `None` for a null pointer.
    ///
    /// # Safety
    ///
    /// `pointer` must be null or point at a NUL-terminated string that stays valid for the call.
    unsafe fn read_c_string(pointer: *const c_char) -> Option<String> {
        if pointer.is_null() {
            return None;
        }
        Some(CStr::from_ptr(pointer).to_string_lossy().into_owned())
    }

    /// Runs `body` with a live pasteboard handle, destroying it afterwards.
    ///
    /// The pasteboard is created per call rather than cached: it is a handle onto a system
    /// service, and holding one past the call would keep the service connection alive for the
    /// whole process lifetime — a resource the caller never asked for.
    fn with_pasteboard<R>(body: impl FnOnce(*mut OhPasteboard) -> R) -> Option<R> {
        let pasteboard = unsafe { OH_Pasteboard_Create() };
        if pasteboard.is_null() {
            log::error!("[harmony pasteboard] OH_Pasteboard_Create returned null");
            return None;
        }
        let result = body(pasteboard);
        unsafe { OH_Pasteboard_Destroy(pasteboard) };
        Some(result)
    }

    /// The plain text of `record`, through the UDS the header requires the caller to supply.
    fn record_html(record: *mut OhUdmfRecord) -> Option<String> {
        let uds = unsafe { OH_UdsHtml_Create() };
        if uds.is_null() {
            log::error!("[harmony pasteboard] OH_UdsHtml_Create returned null");
            return None;
        }
        let status = unsafe { OH_UdmfRecord_GetHtml(record, uds) };
        let html = if status == OHOS_OK {
            unsafe { read_c_string(OH_UdsHtml_GetContent(uds)) }
        } else {
            None
        };
        unsafe { OH_UdsHtml_Destroy(uds) };
        html
    }

    /// Whether the pasteboard currently holds data of `data_type`.
    pub(super) fn has_type(data_type: &str) -> bool {
        let type_buffer = c_string(data_type);
        with_pasteboard(|pasteboard| unsafe {
            OH_Pasteboard_HasType(pasteboard, type_buffer.as_ptr().cast())
        })
        .unwrap_or(false)
    }

    /// Builds `Uds → Record → Data` around one representation and publishes it.
    ///
    /// `fill` receives a live `OH_UdmfRecord` and attaches its representation to it;
    /// `read_back` is the accessor the confirmation read uses, and `expect` is the value it must
    /// find.
    fn publish(
        fill: impl FnOnce(*mut OhUdmfRecord) -> bool,
        read_back: fn() -> Option<String>,
        expect: &str,
    ) -> bool {
        let data = unsafe { OH_UdmfData_Create() };
        let record = unsafe { OH_UdmfRecord_Create() };
        if data.is_null() || record.is_null() {
            log::error!("[harmony pasteboard] UDMF create failed (data or record was null)");
            if !data.is_null() {
                unsafe { OH_UdmfData_Destroy(data) };
            }
            if !record.is_null() {
                unsafe { OH_UdmfRecord_Destroy(record) };
            }
            return false;
        }

        let attached = fill(record);
        let status = if attached { unsafe { OH_UdmfData_AddRecord(data, record) } } else { -1 };
        let written = status == OHOS_OK
            && with_pasteboard(|pasteboard| unsafe { OH_Pasteboard_SetData(pasteboard, data) })
                .is_some_and(|set_status| set_status == OHOS_OK);

        // The caller owns all three objects (`OH_UdmfData_AddRecord` and
        // `OH_Pasteboard_SetData` copy what they need), so they go back regardless of the
        // outcome — a refused write must not leak the payload it refused.
        unsafe {
            OH_UdmfData_Destroy(data);
            OH_UdmfRecord_Destroy(record);
        }

        if !written {
            return false;
        }
        // `OH_Pasteboard_SetData` says the request was accepted; only the read says the clipboard
        // holds it.
        let confirmed = read_back().is_some_and(|stored| stored == expect);
        if !confirmed {
            log::warn!(
                "[harmony pasteboard] OH_Pasteboard_SetData answered OK but the clipboard does \
                 not hold the value that was written"
            );
        }
        confirmed
    }

    /// Publishes `value` as plain text.
    pub(super) fn set_text(value: &str) -> bool {
        let content = c_string(value);
        publish(
            |record| {
                let uds = unsafe { OH_UdsPlainText_Create() };
                if uds.is_null() {
                    return false;
                }
                let set = unsafe { OH_UdsPlainText_SetContent(uds, content.as_ptr().cast()) };
                let added =
                    set == OHOS_OK && unsafe { OH_UdmfRecord_AddPlainText(record, uds) } == OHOS_OK;
                unsafe { OH_UdsPlainText_Destroy(uds) };
                added
            },
            self::text,
            value,
        )
    }

    /// Publishes `html` with `plain` as its plain-text fallback.
    pub(super) fn set_html(html_text: &str, plain: &str) -> bool {
        let html_buffer = c_string(html_text);
        let plain_buffer = c_string(plain);
        publish(
            |record| {
                let uds = unsafe { OH_UdsHtml_Create() };
                if uds.is_null() {
                    return false;
                }
                let set_content =
                    unsafe { OH_UdsHtml_SetContent(uds, html_buffer.as_ptr().cast()) };
                // The plain fallback is what a receiver that cannot render markup falls back to;
                // a pasteboard that took the markup but refused it still holds readable text.
                let set_plain =
                    unsafe { OH_UdsHtml_SetPlainContent(uds, plain_buffer.as_ptr().cast()) };
                let added = set_content == OHOS_OK
                    && set_plain == OHOS_OK
                    && unsafe { OH_UdmfRecord_AddHtml(record, uds) } == OHOS_OK;
                unsafe { OH_UdsHtml_Destroy(uds) };
                added
            },
            self::html,
            html_text,
        )
    }

    /// The pasteboard's plain text, or `None` when it holds none.
    pub(super) fn text() -> Option<String> {
        let data = read_data()?;
        let text = {
            let uds = unsafe { OH_UdsPlainText_Create() };
            if uds.is_null() {
                None
            } else {
                let status = unsafe { OH_UdmfData_GetPrimaryPlainText(data, uds) };
                let text = if status == OHOS_OK {
                    unsafe { read_c_string(OH_UdsPlainText_GetContent(uds)) }
                } else {
                    None
                };
                unsafe { OH_UdsPlainText_Destroy(uds) };
                text
            }
        };
        unsafe { OH_UdmfData_Destroy(data) };
        text
    }

    /// The pasteboard's HTML, or `None` when it holds none.
    pub(super) fn html() -> Option<String> {
        let data = read_data()?;
        let html = {
            // The record is borrowed from `data` — the OpenHarmony sample code destroys only the
            // data it got from `OH_Pasteboard_GetData`, so destroying this one would be a double
            // free.
            let record = unsafe { OH_UdmfData_GetRecord(data, 0) };
            if record.is_null() {
                None
            } else {
                record_html(record)
            }
        };
        unsafe { OH_UdmfData_Destroy(data) };
        html
    }

    /// `OH_Pasteboard_GetData`, reporting the service's own status.
    fn read_data() -> Option<*mut OhUdmfData> {
        with_pasteboard(|pasteboard| {
            let mut status = -1i32;
            let data = unsafe { OH_Pasteboard_GetData(pasteboard, &mut status) };
            if status != OHOS_OK || data.is_null() {
                log::warn!("[harmony pasteboard] OH_Pasteboard_GetData answered status {status}");
                return None;
            }
            Some(data)
        })
        .flatten()
    }

    /// Names the bound service, for the log line a refusal writes.
    pub(super) fn describe() -> &'static str {
        "OH_Pasteboard over UDMF (libpasteboard + libudmf)"
    }
}

#[cfg(not(target_env = "ohos"))]
mod sys {
    //! Host stand-in: the same surface, with nothing behind it.
    //!
    //! The declarations in the real module resolve against `libpasteboard`/`libudmf`, which exist
    //! only in the OpenHarmony sysroot — linking them on a host build would fail. This version
    //! keeps the module compilable (and its non-FFI logic testable) without claiming a pasteboard
    //! that is not there: [`available`] is `false`, so no caller reaches the rest.

    use crate::compat::String;

    /// Always `false`: a host has no OpenHarmony pasteboard.
    pub(super) fn available() -> bool {
        false
    }

    /// Unreachable in practice; present so the two modules have one shape.
    pub(super) fn has_type(_data_type: &str) -> bool {
        false
    }

    /// Unreachable in practice.
    pub(super) fn set_text(_text: &str) -> bool {
        false
    }

    /// Unreachable in practice.
    pub(super) fn set_html(_html: &str, _plain: &str) -> bool {
        false
    }

    /// Unreachable in practice.
    pub(super) fn text() -> Option<String> {
        None
    }

    /// Unreachable in practice.
    pub(super) fn html() -> Option<String> {
        None
    }

    /// Names the build, for the same log line the real module writes.
    pub(super) fn describe() -> &'static str {
        "not built for an OpenHarmony target"
    }
}

/// Whether this build links the OpenHarmony pasteboard.
///
/// `true` exactly when the [`sys`] module above is the real one — the same `cfg` the trait
/// methods and the capability flag are produced from, so the flag and the bodies cannot drift.
pub(crate) const PASTEBOARD_AVAILABLE: bool = cfg!(target_env = "ohos");

/// Publishes `text` on the system pasteboard.
///
/// `None` means "this build has no OpenHarmony pasteboard" — a host build, where the caller
/// should fall back to its own record. `Some(written)` is the pasteboard's own answer, confirmed
/// by a read-back.
pub(crate) fn set_text(text: &str) -> Option<bool> {
    if !sys::available() {
        return None;
    }
    Some(sys::set_text(text))
}

/// Publishes an HTML fragment (with its plain fallback) on the system pasteboard.
pub(crate) fn set_html(html: &str, plain: &str) -> Option<bool> {
    if !sys::available() {
        return None;
    }
    Some(sys::set_html(html, plain))
}

/// The system pasteboard's plain text, or `None` when it holds none.
pub(crate) fn text() -> Option<String> {
    if !sys::available() {
        return None;
    }
    sys::text()
}

/// The system pasteboard's HTML, or `None` when it holds none.
pub(crate) fn html() -> Option<String> {
    if !sys::available() {
        return None;
    }
    sys::html()
}

/// Whether the system pasteboard holds data of `content_type` (a MIME type).
pub(crate) fn has_format(content_type: &str) -> bool {
    if !sys::available() {
        return false;
    }
    match content_type {
        PLAIN_TEXT_TYPE | HTML_TYPE => sys::has_type(content_type),
        // Every other type this crate can name (RTF, images, file lists) has no UDMF
        // representation here, and answering `true` for one would be a claim with nothing
        // behind it.
        _ => false,
    }
}

/// The OpenHarmony host clipboard.
///
/// # What it carries, and what it does not
///
/// Text and HTML — the same pair the macOS, iOS and Android backends implement, and the pair
/// UDMF's `OH_UdsPlainText`/`OH_UdsHtml` represent directly. Images, RTF and file lists are
/// refused (`false` / `None`) rather than silently dropped: an image would need a
/// `OH_UdsPixelMap` and a `OH_UdsArrayBuffer` handshake with the ArkTS side, and RTF has no UDS
/// at all. A body that returned `true` without it is the fake-success shape this crate's
/// principles forbid.
pub struct HarmonyClipboard;

impl HarmonyClipboard {
    /// Creates the backend. It holds no state: the system pasteboard is the state.
    pub fn new() -> Self {
        Self
    }
}

impl Default for HarmonyClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl RichClipboardBackend for HarmonyClipboard {
    fn set_contents(&self, content: ClipboardContent) -> bool {
        let written = match content {
            ClipboardContent::Text(text) => set_text(&text),
            ClipboardContent::Html { html, plain } => set_html(&html, &plain),
            other => {
                log::warn!(
                    "[harmony pasteboard] {} is not carried by this backend ({}); the write is \
                     reported as refused rather than pretended",
                    other.content_type(),
                    sys::describe()
                );
                return false;
            }
        };
        written.unwrap_or(false)
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

    /// The module must not claim a pasteboard on a build that has none.
    ///
    /// This is the flag/method pair the capability check cares about, asserted in the direction
    /// the host can actually observe: on a host build `PASTEBOARD_AVAILABLE` is `false` and every
    /// accessor answers "no host clipboard" instead of an empty string that reads like a real
    /// empty clipboard.
    #[test]
    fn a_build_without_the_pasteboard_answers_none_rather_than_an_empty_clipboard() {
        if PASTEBOARD_AVAILABLE {
            // On an OpenHarmony target the real service answers; asserting the host contract here
            // would be asserting the opposite of what this build is.
            return;
        }
        assert_eq!(set_text("x"), None);
        assert_eq!(set_html("<b>x</b>", "x"), None);
        assert_eq!(text(), None);
        assert_eq!(html(), None);
        assert!(!has_format(PLAIN_TEXT_TYPE));
        assert!(!has_format("image/png"));
    }

    /// The unused-content-type answer is `false`, not an error or a panic.
    #[test]
    fn an_unrepresentable_content_type_is_refused() {
        assert!(!has_format("text/rtf"));
        assert!(!has_format("text/uri-list"));
        assert!(!has_format("image/png"));
    }
}
