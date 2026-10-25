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

use crate::compat::{String, Vec};
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

    use crate::compat::{MiniToString, String, Vec};
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

    /// Opaque `OH_Utd` (a unified type descriptor).
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub(super) struct OhUtd {
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

        /// Attaches one application-defined representation to a record.
        ///
        /// `type_id` is a UTD type identifier (`udmf_meta.h` names the built-in ones). UDMF's
        /// general-entry channel is the only one that carries an opaque byte payload — the plain
        /// text and HTML UDS types each impose their own encoding.
        fn OH_UdmfRecord_AddGeneralEntry(
            record: *mut OhUdmfRecord,
            type_id: *const c_char,
            entry: *mut u8,
            count: u32,
        ) -> i32;
        fn OH_UdmfRecord_GetGeneralEntry(
            record: *mut OhUdmfRecord,
            type_id: *const c_char,
            entry: *mut *mut u8,
            count: *mut u32,
        ) -> i32;
        fn OH_UdmfRecord_GetTypes(record: *mut OhUdmfRecord, count: *mut u32) -> *mut *mut c_char;

        /// Maps a MIME type to the UTD type identifiers the platform registers for it.
        ///
        /// An application-defined MIME has no UTD registration, so the list is empty and the
        /// caller falls back to using the MIME name itself as the type identifier.
        fn OH_Utd_GetTypesByMimeType(
            mime_type: *const c_char,
            count: *mut u32,
        ) -> *const *const c_char;
        fn OH_Utd_DestroyStringList(list: *const *const c_char, count: u32);

        /// The reverse mapping, used when the pasteboard reports a registered type by its UTD id
        /// rather than by the MIME name the caller wrote.
        fn OH_Utd_Create(type_id: *const c_char) -> *mut OhUtd;
        fn OH_Utd_Destroy(utd: *mut OhUtd);
        fn OH_Utd_GetMimeTypes(utd: *mut OhUtd, count: *mut u32) -> *const *const c_char;

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
        // SAFETY: FFI constructor; the returned handle is checked for null before use.
        let pasteboard = unsafe { OH_Pasteboard_Create() };
        if pasteboard.is_null() {
            log::error!("[harmony pasteboard] OH_Pasteboard_Create returned null");
            return None;
        }
        let result = body(pasteboard);
        // SAFETY: `pasteboard` is the live handle created above and is not touched afterwards.
        unsafe { OH_Pasteboard_Destroy(pasteboard) };
        Some(result)
    }

    /// The plain text of `record`, through the UDS the header requires the caller to supply.
    fn record_html(record: *mut OhUdmfRecord) -> Option<String> {
        // SAFETY: FFI constructor; the returned object is checked for null below.
        let uds = unsafe { OH_UdsHtml_Create() };
        if uds.is_null() {
            log::error!("[harmony pasteboard] OH_UdsHtml_Create returned null");
            return None;
        }
        // SAFETY: `record` is borrowed from the caller and `uds` is the live object created above.
        let status = unsafe { OH_UdmfRecord_GetHtml(record, uds) };
        let html = if status == OHOS_OK {
            // SAFETY: `uds` is live; the content pointer it returns stays valid until `uds` is destroyed.
            unsafe { read_c_string(OH_UdsHtml_GetContent(uds)) }
        } else {
            None
        };
        // SAFETY: `uds` is the live object created above and is not used afterwards.
        unsafe { OH_UdsHtml_Destroy(uds) };
        html
    }

    /// Whether the pasteboard currently holds data of `data_type`.
    pub(super) fn has_type(data_type: &str) -> bool {
        let type_buffer = c_string(data_type);
        // SAFETY: `pasteboard` is live for the closure and `type_buffer` is NUL-terminated.
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
        // SAFETY: FFI constructor; the returned object is checked for null below.
        let data = unsafe { OH_UdmfData_Create() };
        // SAFETY: FFI constructor; the returned object is checked for null below.
        let record = unsafe { OH_UdmfRecord_Create() };
        if data.is_null() || record.is_null() {
            log::error!("[harmony pasteboard] UDMF create failed (data or record was null)");
            if !data.is_null() {
                // SAFETY: `data` is non-null and was created in this call.
                unsafe { OH_UdmfData_Destroy(data) };
            }
            if !record.is_null() {
                // SAFETY: `record` is non-null and was created in this call.
                unsafe { OH_UdmfRecord_Destroy(record) };
            }
            return false;
        }

        let attached = fill(record);
        // SAFETY: `data` and `record` are live objects owned by this function.
        let status = if attached { unsafe { OH_UdmfData_AddRecord(data, record) } } else { -1 };
        let written = status == OHOS_OK
// SAFETY: the pasteboard handle is live for the closure; `data` is a live UDMF object.
            && with_pasteboard(|pasteboard| unsafe { OH_Pasteboard_SetData(pasteboard, data) })
                .is_some_and(|set_status| set_status == OHOS_OK);

        // The caller owns all three objects (`OH_UdmfData_AddRecord` and
        // `OH_Pasteboard_SetData` copy what they need), so they go back regardless of the
        // outcome — a refused write must not leak the payload it refused.
        // SAFETY: both handles were created in this call and are not used after this drop.
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
                // SAFETY: FFI constructor; the returned object is checked for null below.
                let uds = unsafe { OH_UdsPlainText_Create() };
                if uds.is_null() {
                    return false;
                }
                // SAFETY: `uds` is live and `content` is a NUL-terminated local buffer outliving the call.
                let set = unsafe { OH_UdsPlainText_SetContent(uds, content.as_ptr().cast()) };
                let added =
// SAFETY: `record` is the live record passed to `fill`, and `uds` is live.
                    set == OHOS_OK && unsafe { OH_UdmfRecord_AddPlainText(record, uds) } == OHOS_OK;
                // SAFETY: `uds` is the live object created above and is not used afterwards.
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
                // SAFETY: FFI constructor; the returned object is checked for null below.
                let uds = unsafe { OH_UdsHtml_Create() };
                if uds.is_null() {
                    return false;
                }
                let set_content =
// SAFETY: `uds` is live and `html_buffer` is a NUL-terminated local buffer outliving the call.
                    unsafe { OH_UdsHtml_SetContent(uds, html_buffer.as_ptr().cast()) };
                // The plain fallback is what a receiver that cannot render markup falls back to;
                // a pasteboard that took the markup but refused it still holds readable text.
                let set_plain =
// SAFETY: `uds` is live and `plain_buffer` is a NUL-terminated local buffer outliving the call.
                    unsafe { OH_UdsHtml_SetPlainContent(uds, plain_buffer.as_ptr().cast()) };
                let added = set_content == OHOS_OK
                    && set_plain == OHOS_OK
// SAFETY: `record` is the live record passed to `fill`, and `uds` is live.
                    && unsafe { OH_UdmfRecord_AddHtml(record, uds) } == OHOS_OK;
                // SAFETY: `uds` is the live object created above and is not used afterwards.
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
            // SAFETY: FFI constructor; the returned object is checked for null below.
            let uds = unsafe { OH_UdsPlainText_Create() };
            if uds.is_null() {
                None
            } else {
                // SAFETY: `data` is live and `uds` is the live object created above.
                let status = unsafe { OH_UdmfData_GetPrimaryPlainText(data, uds) };
                let text = if status == OHOS_OK {
                    // SAFETY: `uds` is live; the content pointer it returns stays valid until `uds` is destroyed.
                    unsafe { read_c_string(OH_UdsPlainText_GetContent(uds)) }
                } else {
                    None
                };
                // SAFETY: `uds` is the live object created above and is not used afterwards.
                unsafe { OH_UdsPlainText_Destroy(uds) };
                text
            }
        };
        // SAFETY: `data` is the live object returned by `read_data` and is not used afterwards.
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
            // SAFETY: `data` is live; the record is borrowed from it and must not be destroyed here.
            let record = unsafe { OH_UdmfData_GetRecord(data, 0) };
            if record.is_null() {
                None
            } else {
                record_html(record)
            }
        };
        // SAFETY: `data` is the live object returned by `read_data` and is not used afterwards.
        unsafe { OH_UdmfData_Destroy(data) };
        html
    }

    /// The type identifiers that can name `content_type` on the pasteboard.
    ///
    /// The MIME name itself is tried first so an application-defined type is carried verbatim
    /// when the platform accepts it. UTD's registered identifiers follow, which is what makes a
    /// MIME the platform already knows (for example `text/uri-list`) interoperable with other
    /// OpenHarmony applications.
    fn type_ids_for_mime(content_type: &str) -> Vec<String> {
        let mut ids = Vec::new();
        ids.push(content_type.to_string());

        let mime_buffer = c_string(content_type);
        let mut count = 0u32;
        // SAFETY: the MIME buffer is NUL-terminated and `count` is writable for the call.
        let list = unsafe { OH_Utd_GetTypesByMimeType(mime_buffer.as_ptr().cast(), &mut count) };
        if !list.is_null() {
            for index in 0..count as usize {
                // SAFETY: `list` is non-null with `count` entries, and `index < count`.
                let pointer = unsafe { *list.add(index) };
                // SAFETY: `pointer` is either null or a NUL-terminated id owned by `list`.
                if let Some(id) = unsafe { read_c_string(pointer) } {
                    if !ids.iter().any(|known| known == &id) {
                        ids.push(id);
                    }
                }
            }
            // SAFETY: `list`/`count` are exactly the values returned by `OH_Utd_GetTypesByMimeType`.
            unsafe { OH_Utd_DestroyStringList(list, count) };
        }
        ids
    }

    /// Publishes `data` under `content_type` through a UDMF general entry.
    ///
    /// The first type identifier the platform accepts is used; the read-back then asks for the
    /// same set of identifiers, so a write that landed under a UTD id is still found by the MIME
    /// the caller named.
    pub(super) fn set_custom(content_type: &str, data: &[u8]) -> bool {
        let Ok(count) = u32::try_from(data.len()) else {
            log::warn!(
                "[harmony pasteboard] {content_type}: payload is larger than UDMF's 32-bit entry \
                 size and was refused"
            );
            return false;
        };
        let ids = type_ids_for_mime(content_type);
        let payload = data.to_vec();
        // SAFETY: FFI constructor; the returned object is checked for null below.
        let data_handle = unsafe { OH_UdmfData_Create() };
        // SAFETY: FFI constructor; the returned object is checked for null below.
        let record = unsafe { OH_UdmfRecord_Create() };
        if data_handle.is_null() || record.is_null() {
            log::error!("[harmony pasteboard] UDMF create failed (data or record was null)");
            if !data_handle.is_null() {
                // SAFETY: `data_handle` is non-null and was created in this call.
                unsafe { OH_UdmfData_Destroy(data_handle) };
            }
            if !record.is_null() {
                // SAFETY: `record` is non-null and was created in this call.
                unsafe { OH_UdmfRecord_Destroy(record) };
            }
            return false;
        }

        let mut attached = false;
        for id in &ids {
            let type_buffer = c_string(id);
            // SAFETY: `record` is live; the type and payload buffers outlive the call.
            let status = unsafe {
                OH_UdmfRecord_AddGeneralEntry(
                    record,
                    type_buffer.as_ptr().cast(),
                    payload.as_ptr() as *mut u8,
                    count,
                )
            };
            if status == OHOS_OK {
                attached = true;
                break;
            }
        }

        let status =
// SAFETY: `data_handle` and `record` are live objects owned by this function.
            if attached { unsafe { OH_UdmfData_AddRecord(data_handle, record) } } else { -1 };
        let written = status == OHOS_OK
// SAFETY: the pasteboard handle is live for the closure; `data_handle` is a live UDMF object.
            && with_pasteboard(|pasteboard| unsafe {
                OH_Pasteboard_SetData(pasteboard, data_handle)
            })
            .is_some_and(|set_status| set_status == OHOS_OK);

        // SAFETY: both handles were created in this call and are not used after this drop.
        unsafe {
            OH_UdmfData_Destroy(data_handle);
            OH_UdmfRecord_Destroy(record);
        }

        if !written {
            return false;
        }
        let confirmed = custom_for_type(content_type).is_some_and(|stored| stored == payload);
        if !confirmed {
            log::warn!(
                "[harmony pasteboard] OH_Pasteboard_SetData answered OK but the custom type \
                 {content_type} does not hold the bytes that were written"
            );
        }
        confirmed
    }

    /// The bytes the pasteboard holds under `content_type`, or `None` when it holds none.
    pub(super) fn custom_for_type(content_type: &str) -> Option<Vec<u8>> {
        let ids = type_ids_for_mime(content_type);
        let data = read_data()?;
        let bytes = {
            // SAFETY: `data` is live; the record is borrowed from it and must not be destroyed here.
            let record = unsafe { OH_UdmfData_GetRecord(data, 0) };
            if record.is_null() {
                None
            } else {
                ids.iter().find_map(|id| record_general_entry(record, id))
            }
        };
        // SAFETY: `data` is the live object returned by `read_data` and is not used afterwards.
        unsafe { OH_UdmfData_Destroy(data) };
        bytes
    }

    /// Whether the pasteboard currently holds data under `content_type`.
    pub(super) fn has_custom(content_type: &str) -> bool {
        let ids = type_ids_for_mime(content_type);
        with_pasteboard(|pasteboard| {
            ids.iter().any(|id| {
                let type_buffer = c_string(id);
                // SAFETY: `pasteboard` is live for the closure and `type_buffer` is NUL-terminated.
                unsafe { OH_Pasteboard_HasType(pasteboard, type_buffer.as_ptr().cast()) }
            })
        })
        .unwrap_or(false)
    }

    /// The first application-defined entry on the pasteboard, as `(type, bytes)`.
    ///
    /// The two fixed UDS types are skipped: they have their own accessors, and a caller asking
    /// for "the custom payload" must not receive the plain-text fallback of an HTML clip.
    pub(super) fn first_custom() -> Option<(String, Vec<u8>)> {
        let data = read_data()?;
        let found = {
            // SAFETY: `data` is live; the record is borrowed from it and must not be destroyed here.
            let record = unsafe { OH_UdmfData_GetRecord(data, 0) };
            if record.is_null() {
                None
            } else {
                first_general_entry(record)
            }
        };
        // SAFETY: `data` is the live object returned by `read_data` and is not used afterwards.
        unsafe { OH_UdmfData_Destroy(data) };
        found
    }

    /// Reads one general entry from a record.
    fn record_general_entry(record: *mut OhUdmfRecord, type_id: &str) -> Option<Vec<u8>> {
        let type_buffer = c_string(type_id);
        let mut entry: *mut u8 = core::ptr::null_mut();
        let mut count = 0u32;
        // SAFETY: `record` is live; `type_buffer` is NUL-terminated and the out-params are writable.
        let status = unsafe {
            OH_UdmfRecord_GetGeneralEntry(
                record,
                type_buffer.as_ptr().cast(),
                &mut entry,
                &mut count,
            )
        };
        if status != OHOS_OK || entry.is_null() || count == 0 {
            return None;
        }
        // The pointer addresses memory owned by the record; copying is the only safe use, and the
        // record (and the data that owns it) is destroyed by the caller after this returns.
        // SAFETY: `entry`/`count` describe a buffer owned by `record`, which outlives this copy.
        Some(unsafe { core::slice::from_raw_parts(entry, count as usize) }.to_vec())
    }

    /// Enumerates a record's types and returns the first general entry with bytes.
    fn first_general_entry(record: *mut OhUdmfRecord) -> Option<(String, Vec<u8>)> {
        let mut count = 0u32;
        // The type list is borrowed from the record: the SDK declares no destructor for it (unlike
        // `OH_Utd_DestroyStringList`), and the record is destroyed by the caller after this returns.
        // SAFETY: `record` is live and `count` is a writable out-parameter.
        let types = unsafe { OH_UdmfRecord_GetTypes(record, &mut count) };
        if types.is_null() {
            return None;
        }
        for index in 0..count as usize {
            // SAFETY: `types` is non-null with `count` entries, and `index < count`.
            let type_pointer = unsafe { *types.add(index) };
            // SAFETY: `type_pointer` is either null or a NUL-terminated id owned by `types`.
            let Some(type_id) = (unsafe { read_c_string(type_pointer) }) else {
                continue;
            };
            if type_id == "general.plain-text" || type_id == "general.html" {
                continue;
            }
            if let Some(bytes) = record_general_entry(record, &type_id) {
                // The pasteboard may report a registered type by its UTD id. Answering the MIME
                // the type maps to keeps `ClipboardContent::content_type()`'s contract (a MIME)
                // and lets the caller ask for the same name it wrote.
                let content_type = mime_for_type_id(&type_id).unwrap_or(type_id);
                return Some((content_type, bytes));
            }
        }
        None
    }

    /// The first MIME type a UTD type id maps to, if any.
    fn mime_for_type_id(type_id: &str) -> Option<String> {
        let type_buffer = c_string(type_id);
        // SAFETY: FFI constructor; the id buffer is NUL-terminated and the result is null-checked.
        let utd = unsafe { OH_Utd_Create(type_buffer.as_ptr().cast()) };
        if utd.is_null() {
            return None;
        }
        let mut count = 0u32;
        // SAFETY: `utd` is non-null and live; `count` is a writable out-parameter.
        let mimes = unsafe { OH_Utd_GetMimeTypes(utd, &mut count) };
        let mime =
// SAFETY: `mimes` is non-null with at least one entry, as checked on this line.
            if !mimes.is_null() && count > 0 { unsafe { read_c_string(*mimes) } } else { None };
        if !mimes.is_null() {
            // SAFETY: `mimes`/`count` are the values returned by `OH_Utd_GetMimeTypes`.
            unsafe { OH_Utd_DestroyStringList(mimes, count) };
        }
        // SAFETY: `utd` is the live object created above and is not used afterwards.
        unsafe { OH_Utd_Destroy(utd) };
        mime
    }

    /// `OH_Pasteboard_GetData`, reporting the service's own status.
    fn read_data() -> Option<*mut OhUdmfData> {
        with_pasteboard(|pasteboard| {
            let mut status = -1i32;
            // SAFETY: `pasteboard` is live for the closure and `status` is a writable out-parameter.
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

    use crate::compat::{String, Vec};

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
    pub(super) fn set_custom(_content_type: &str, _data: &[u8]) -> bool {
        false
    }

    /// Unreachable in practice.
    pub(super) fn custom_for_type(_content_type: &str) -> Option<Vec<u8>> {
        None
    }

    /// Unreachable in practice.
    pub(super) fn has_custom(_content_type: &str) -> bool {
        false
    }

    /// Unreachable in practice.
    pub(super) fn first_custom() -> Option<(String, Vec<u8>)> {
        None
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
        // Every other type this crate can name (RTF, images, file lists) has no UDS
        // representation here. Application-defined types go through UDMF's general entry, which
        // is the one channel that carries opaque bytes.
        _ => sys::has_custom(content_type),
    }
}

/// Publishes `data` under an application-defined MIME type.
///
/// `None` means this build has no OpenHarmony pasteboard; `Some(written)` is the pasteboard's
/// answer, confirmed by a read-back.
pub(crate) fn set_contents_with_type(content_type: &str, data: &[u8]) -> Option<bool> {
    if !sys::available() {
        return None;
    }
    Some(sys::set_custom(content_type, data))
}

/// The bytes the pasteboard holds under `content_type`, or `None` when it holds none.
pub(crate) fn get_contents_for_type(content_type: &str) -> Option<Vec<u8>> {
    if !sys::available() {
        return None;
    }
    match content_type {
        PLAIN_TEXT_TYPE => text().map(String::into_bytes),
        HTML_TYPE => html().map(String::into_bytes),
        _ => sys::custom_for_type(content_type),
    }
}

/// The first application-defined payload on the pasteboard, if any.
pub(crate) fn first_custom() -> Option<(String, Vec<u8>)> {
    if !sys::available() {
        return None;
    }
    sys::first_custom()
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
            ClipboardContent::Custom { content_type, data } => {
                set_contents_with_type(&content_type, &data)
            }
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
        // Custom first: a general entry carries opaque bytes, and the text accessors cannot see
        // it. The reverse order would report a custom clip as "nothing" and hide its type.
        if let Some((content_type, data)) = first_custom() {
            return Some(ClipboardContent::Custom { content_type, data });
        }
        if let Some(html) = html() {
            return Some(ClipboardContent::Html { html, plain: text().unwrap_or_default() });
        }
        text().map(ClipboardContent::Text)
    }

    fn has_format(&self, content_type: &str) -> bool {
        has_format(content_type)
    }

    /// Stores bytes under an application-defined MIME type.
    fn set_contents_with_type(&self, content_type: &str, data: &[u8]) -> bool {
        set_contents_with_type(content_type, data).unwrap_or(false)
    }

    /// Reads the bytes the pasteboard holds under `content_type`.
    fn get_contents_for_type(&self, content_type: &str) -> Option<Vec<u8>> {
        get_contents_for_type(content_type)
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
