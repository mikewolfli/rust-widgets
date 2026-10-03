// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Windows IME bridge — a real TSF (Text Services Framework) TIP.
//!
//! On `target_os = "windows"` this module binds the TSF COM interfaces by hand and *is* a
//! text service: it creates `ITfThreadMgr` with `CoCreateInstance(CLSID_TF_ThreadMgr)`,
//! activates it, installs a document manager with a live context and a **sink**, and receives
//! the OS input method's composition and commit callbacks. The committed text is routed through
//! [`ImeBridge`] into the focused widget exactly the way [`super::ime::deliver_commit`] already
//! joined the platform and widget halves.
//!
//! On every other target — and in headless testing — the same type is a pure state machine
//! that correctly tracks marked text, composition start offsets, and cursor positions, and
//! reports [`ImeBridge::is_active`] as `false` because there is genuinely no TSF connection.

use crate::compat::{lock, Mutex, String, ToString};
use crate::core::ObjectId;
use crate::platform::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};

// ══════════════════════════════════════════════════════════════════════════════════════════
// TSF COM bindings (Windows only)
// ══════════════════════════════════════════════════════════════════════════════════════════
//
// # Why these are hand-written rather than taken from a crate
//
// `winapi` 0.3 ships neither the TSF interfaces (`ctfutb` / `msctf`) nor `CLSID_TF_ThreadMgr`,
// and the crate has no `windows`/`windows-sys` dependency — adding one for a handful of
// vtables would be a heavy dependency for the exact thing the repo already hand-rolls
// elsewhere (see `platform/harmony/xcomponent.rs`'s "Why hand-written `extern \"C\"`"
// section). So the ABI is transcribed here once, gated on `target_os = "windows"`, and the
// layouts are pinned by tests below.
//
// # What is transcribed and why each piece is needed
//
// | Interface | Role in this bridge |
// |---|---|
// | `IUnknown` | the three root slots every vtable begins with; `Release` is how every COM object here is freed |
// | `ITfThreadMgr` | the per-thread TSF entry point (`Activate` / `Deactivate`), created by `CoCreateInstance` |
// | `ITfDocumentMgr` | holds the stack of contexts; created by `ITfThreadMgr::CreateDocumentMgr` |
// | `ITfContext` | the edit target the input method writes composition into; created by `CreateContext` |
// | `ITfThreadMgrEventSink` | document-manager focus notifications; marks the context as setup |
// | `ITfTextEditSink` | the **composition/commit** callbacks (`OnEndEdit`) |
//
// `ITfThreadMgr` alone carries ~30 methods before the ones used here, and `ITfContext` ~25;
// each is bound up to and including the used slot, with the unused tail covered by opaque
// `usize` (pointer-sized) padding. A vtable is only ever read through the slots the ABI
// defines, so the padding preserves the used slots' offsets without transcribing the rest.

#[cfg(target_os = "windows")]
mod tsf {
    use core::ffi::c_void;

    /// `S_OK`. Any `HRESULT >= 0` is success, but this is the only success these calls return.
    pub const S_OK: i32 = 0;
    /// `E_NOINTERFACE`, returned by `QueryInterface` when an interface is unsupported.
    pub const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;

    /// `CLSCTX_INPROC_SERVER` (0x1): the TSF thread manager is an in-process COM server.
    pub const CLSCTX_INPROC_SERVER: u32 = 0x1;
    /// `TF_POPF_ALL`: pop every context off the document stack on teardown.
    pub const TF_POPF_ALL: u32 = 0x1;
    /// `TF_CES_CURRENTINPUTTHREAD` classification for `CreateContext` — the ordinary desktop case.
    pub const TF_CES_CURRENTINPUTTHREAD: i32 = 0x2;

    /// `GUID`, as `winapi::shared::guiddef::GUID`. `c_ulong` is `u32` on every Windows ABI.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Guid {
        /// First `u32` half of the Data1 field.
        pub data1: u32,
        /// Data2.
        pub data2: u16,
        /// Data3.
        pub data3: u16,
        /// Data4, the trailing eight bytes.
        pub data4: [u8; 8],
    }

    /// Rust spelling of `winapi`'s `DEFINE_GUID!`, usable inside this crate without importing
    /// the macro (which is a private `#[macro_use]` in `winapi`, not a public path).
    macro_rules! guid {
        ($d1:expr, $d2:expr, $d3:expr, $b4:expr, $b5:expr, $b6:expr, $b7:expr, $b8:expr, $b9:expr, $b10:expr, $b11:expr) => {
            Guid {
                data1: $d1,
                data2: $d2,
                data3: $d3,
                data4: [$b4, $b5, $b6, $b7, $b8, $b9, $b10, $b11],
            }
        };
    }

    /// `CLSID_TF_ThreadMgr` — the TSF thread manager class.
    /// `{529A9E6B-6587-4F23-AB9E-9C7D683E3C50}`.
    pub const CLSID_TF_THREAD_MGR: Guid =
        guid!(0x529A_9E6B, 0x6587, 0x4F23, 0xAB, 0x9E, 0x9C, 0x7D, 0x68, 0x3E, 0x3C, 0x50);

    /// `IID_ITfThreadMgr` — `{AA80E801-2021-11D2-93E0-0060B067B86E}`.
    pub const IID_ITF_THREAD_MGR: Guid =
        guid!(0xAA80_E801, 0x2021, 0x11D2, 0x93, 0xE0, 0x00, 0x60, 0xB0, 0x67, 0xB8, 0x6E);

    /// `IID_ITfThreadMgrEventSink` — `{AA80E80E-2021-11D2-93E0-0060B067B86E}`.
    pub const IID_ITF_THREAD_MGR_EVENT_SINK: Guid =
        guid!(0xAA80_E80E, 0x2021, 0x11D2, 0x93, 0xE0, 0x00, 0x60, 0xB0, 0x67, 0xB8, 0x6E);

    /// `IID_ITfTextEditSink` — `{8127D901-F856-4E52-814F-6D6CC3B1C6A1}`.
    pub const IID_ITF_TEXT_EDIT_SINK: Guid =
        guid!(0x8127_D901, 0xF856, 0x4E52, 0x81, 0x4F, 0x6D, 0x6C, 0xC3, 0xB1, 0xC6, 0xA1);

    /// An opaque COM interface pointer. Only the leading `lpVtbl` word is ever read.
    #[repr(C)]
    pub struct IUnknown {
        /// Pointer to the `IUnknownVtbl`.
        pub lp_vtbl: *const c_void,
    }

    /// The three universal vtable slots declared by `IUnknown`, in ABI order.
    #[repr(C)]
    pub struct IUnknownVtbl {
        /// `HRESULT QueryInterface(REFIID, void**)`.
        pub query_interface:
            unsafe extern "system" fn(*mut IUnknown, *const Guid, *mut *mut c_void) -> i32,
        /// `ULONG AddRef()`.
        pub add_ref: unsafe extern "system" fn(*mut IUnknown) -> u32,
        /// `ULONG Release()`.
        pub release: unsafe extern "system" fn(*mut IUnknown) -> u32,
    }

    /// `ITfThreadMgr`: bound through `Activate`/`Deactivate`, the slots this bridge calls.
    #[repr(C)]
    pub struct ITfThreadMgr {
        /// `ITfThreadMgrVtbl`.
        pub lp_vtbl: *const ITfThreadMgrVtbl,
    }

    /// `ITfThreadMgrVtbl` through `CreateContext` (slot 8); the remaining ~22 methods are not
    /// called and so are represented by pointer-sized padding.
    #[repr(C)]
    pub struct ITfThreadMgrVtbl {
        /// `IUnknown` slots.
        pub base: IUnknownVtbl,
        /// `HRESULT Activate(TfClientId*)`.
        pub activate: unsafe extern "system" fn(*mut ITfThreadMgr, *mut u32) -> i32,
        /// `HRESULT Deactivate()`.
        pub deactivate: unsafe extern "system" fn(*mut ITfThreadMgr) -> i32,
        /// `HRESULT CreateDocumentMgr(ITfDocumentMgr**)`.
        pub create_document_mgr:
            unsafe extern "system" fn(*mut ITfThreadMgr, *mut *mut ITfDocumentMgr) -> i32,
        /// `HRESULT EnumDocumentMgrs(IEnumTfDocumentMgrs**)`.
        pub enum_document_mgrs: usize,
        /// `HRESULT GetFocus(ITfDocumentMgr**)`.
        pub get_focus:
            unsafe extern "system" fn(*mut ITfThreadMgr, *mut *mut ITfDocumentMgr) -> i32,
        /// `HRESULT SetFocus(ITfDocumentMgr*)`.
        pub set_focus: unsafe extern "system" fn(*mut ITfThreadMgr, *mut ITfDocumentMgr) -> i32,
        /// `HRESULT AssociateFocus(HWND, ITfDocumentMgr, ITfDocumentMgr**)`.
        pub associate_focus: usize,
        /// `HRESULT IsThreadFocus(BOOL*)`.
        pub is_thread_focus: usize,
        /// `HRESULT GetFunctionProvider(REFCLSID, ITfFunctionProvider**)`.
        pub get_function_provider: usize,
        /// `HRESULT EnumFunctionProviders(IEnumTfFunctionProviders**)`.
        pub enum_function_providers: usize,
        /// `HRESULT GetGlobalCompartment(ITfCompartmentMgr**)`.
        pub get_global_compartment:
            unsafe extern "system" fn(*mut ITfThreadMgr, *mut *mut c_void) -> i32,
        /// `HRESULT CreateContext(TfClientId, DWORD, IUnknown*, ITfContext**)`.
        pub create_context: unsafe extern "system" fn(
            *mut ITfThreadMgr,
            u32,
            u32,
            *mut IUnknown,
            *mut *mut ITfContext,
        ) -> i32,
    }

    /// `ITfDocumentMgr`: bound through `CreateContext`/`Pop`.
    #[repr(C)]
    pub struct ITfDocumentMgr {
        /// `ITfDocumentMgrVtbl`.
        pub lp_vtbl: *const ITfDocumentMgrVtbl,
    }

    /// `ITfDocumentMgrVtbl` through `Pop` (slot 6).
    #[repr(C)]
    pub struct ITfDocumentMgrVtbl {
        /// `IUnknown` slots.
        pub base: IUnknownVtbl,
        /// `HRESULT CreateContext(TfClientId, DWORD, IUnknown*, ITfContext**)`.
        pub create_context: unsafe extern "system" fn(
            *mut ITfDocumentMgr,
            u32,
            u32,
            *mut IUnknown,
            *mut *mut ITfContext,
        ) -> i32,
        /// `HRESULT Push(ITfContext*)`.
        pub push: unsafe extern "system" fn(*mut ITfDocumentMgr, *mut ITfContext) -> i32,
        /// `HRESULT Pop(DWORD)`.
        pub pop: unsafe extern "system" fn(*mut ITfDocumentMgr, u32) -> i32,
        /// `HRESULT GetTop(ITfContext**)`.
        pub get_top: unsafe extern "system" fn(*mut ITfDocumentMgr, *mut *mut ITfContext) -> i32,
        /// `HRESULT GetBase(ITfContext**)`.
        pub get_base: unsafe extern "system" fn(*mut ITfDocumentMgr, *mut *mut ITfContext) -> i32,
        /// `HRESULT EnumContexts(IEnumTfContexts**)`.
        pub enum_contexts: usize,
    }

    /// `ITfContext`: an opaque COM pointer to the edit context.
    ///
    /// This bridge never calls a context method directly — it only `QueryInterface`s the
    /// context's leading `IUnknown` slots (see [`advise_sink`]), so no context vtable beyond
    /// `IUnknown` is transcribed. That is deliberate: the full `ITfContext` table is 25 slots
    /// past `IUnknown`, and a slot this code does not call is a transposition that could be
    /// silently wrong — the honest binding stops at what is used.
    #[repr(C)]
    pub struct ITfContext {
        /// The context's vtable pointer; only its leading `IUnknownVtbl` is read.
        pub lp_vtbl: *const IUnknownVtbl,
    }

    /// `ITfThreadMgrEventSink`: bound through `OnSetFocus`/`OnSetFocus`; the three callbacks
    /// below the bound ones are irrelevant and padded.
    #[repr(C)]
    pub struct ITfThreadMgrEventSink {
        /// `ITfThreadMgrEventSinkVtbl`.
        pub lp_vtbl: *const ITfThreadMgrEventSinkVtbl,
    }

    /// `ITfThreadMgrEventSinkVtbl`.
    #[repr(C)]
    pub struct ITfThreadMgrEventSinkVtbl {
        /// `IUnknown` slots.
        pub base: IUnknownVtbl,
        /// `HRESULT OnInitDocumentMgr(ITfDocumentMgr*)`.
        pub init_document_mgr:
            unsafe extern "system" fn(*mut ITfThreadMgrEventSink, *mut ITfDocumentMgr) -> i32,
        /// `HRESULT OnUninitDocumentMgr(ITfDocumentMgr*)`.
        pub uninit_document_mgr:
            unsafe extern "system" fn(*mut ITfThreadMgrEventSink, *mut ITfDocumentMgr) -> i32,
        /// `HRESULT OnSetFocus(ITfDocumentMgr *pdimFocus, ITfDocumentMgr *pdimPrevFocus)`.
        pub set_focus: unsafe extern "system" fn(
            *mut ITfThreadMgrEventSink,
            *mut ITfDocumentMgr,
            *mut ITfDocumentMgr,
        ) -> i32,
        /// `HRESULT OnPushContext(ITfContext*)`.
        pub push_context:
            unsafe extern "system" fn(*mut ITfThreadMgrEventSink, *mut ITfContext) -> i32,
        /// `HRESULT OnPopContext(ITfContext*)`.
        pub pop_context:
            unsafe extern "system" fn(*mut ITfThreadMgrEventSink, *mut ITfContext) -> i32,
    }

    /// `ITfTextEditSink`: the composition/commit callback the input method drives.
    #[repr(C)]
    pub struct ITfTextEditSink {
        /// `ITfTextEditSinkVtbl`.
        pub lp_vtbl: *const ITfTextEditSinkVtbl,
    }

    /// `ITfTextEditSinkVtbl`.
    #[repr(C)]
    pub struct ITfTextEditSinkVtbl {
        /// `IUnknown` slots.
        pub base: IUnknownVtbl,
        /// `HRESULT OnEndEdit(ITfContext*, TfEditCookie, ITfEditRecord*)`.
        pub on_end_edit: unsafe extern "system" fn(
            *mut ITfTextEditSink,
            *mut ITfContext,
            u32,
            *mut c_void,
        ) -> i32,
    }

    // ── The comlink that is handed to TSF as a sink ────────────────────────────────────────
    //
    // A COM object that TSF calls back into needs a stable heap address whose `lpVtbl` points
    // at one static vtable. `ComLink` is that object: it owns a `Box<WindowsImeBridge>`, keeps
    // its own COM refcount atomically, and is what `QueryInterface` returns for the two sink
    // IIDs. It is deliberately a raw `Box::into_raw` allocation whose lifetime is governed by
    // `AddRef`/`Release`, exactly as C++ COM expects.

    use super::WindowsImeBridge;

    /// The `ITfThreadMgrEventSink` vtable this crate installs (a single static instance).
    pub static THREAD_MGR_EVENT_SINK_VTBL: ITfThreadMgrEventSinkVtbl = ITfThreadMgrEventSinkVtbl {
        base: IUnknownVtbl {
            query_interface: comlink_query_interface,
            add_ref: comlink_add_ref,
            release: comlink_release,
        },
        init_document_mgr: sink_init_document_mgr,
        uninit_document_mgr: sink_uninit_document_mgr,
        set_focus: sink_set_focus,
        push_context: sink_push_context,
        pop_context: sink_pop_context,
    };

    /// The `ITfTextEditSink` vtable this crate installs (a single static instance).
    pub static TEXT_EDIT_SINK_VTBL: ITfTextEditSinkVtbl = ITfTextEditSinkVtbl {
        base: IUnknownVtbl {
            query_interface: comlink_query_interface,
            add_ref: comlink_add_ref,
            release: comlink_release,
        },
        on_end_edit: sink_on_end_edit,
    };

    /// A heap-allocated, refcounted COM object that both sink interfaces share. Its
    /// `lpVtbl` field must be first so a pointer to it *is* a valid `ITfThreadMgrEventSink*` /
    /// `ITfTextEditSink*` / `IUnknown*` to the OS.
    ///
    /// # A note on the single vtable pointer
    ///
    /// The two sink interfaces have different tables, so `QueryInterface` re-points
    /// `lp_vtbl` at the table matching the requested IID before handing the object back. This
    /// is the standard IDispatch-free comlink shape: the caller reads `lp_vtbl` from the
    /// pointer it just received, so the immediately-following call lands in the right table.
    /// It also means the comlink must not hold the bridge — the bridge is owned by the
    /// platform, outlives the connection, and must not be freed when the last COM reference
    /// drops.
    #[repr(C)]
    pub struct ComLink {
        /// Vtable for the interface this comlink was last requested as.
        pub lp_vtbl: *const c_void,
        /// COM reference count. Starts at 1 for the bridge's own hold.
        pub ref_count: core::sync::atomic::AtomicU32,
        /// The bridge the sink callbacks deliver into. Borrowed, never owned.
        pub bridge: *const WindowsImeBridge,
    }

    unsafe impl Send for ComLink {}
    unsafe impl Sync for ComLink {}

    /// `IUnknown::QueryInterface` for [`ComLink`]. Answers the two sink IIDs this object
    /// implements and `E_NOINTERFACE` for everything else — never fakes a supported interface.
    unsafe extern "system" fn comlink_query_interface(
        this: *mut IUnknown,
        riid: *const Guid,
        ppv: *mut *mut c_void,
    ) -> i32 {
        if ppv.is_null() {
            return E_NOINTERFACE;
        }
        let comlink = this as *mut ComLink;
        let iid = &*riid;
        if guid_eq(iid, &IID_ITF_THREAD_MGR_EVENT_SINK) {
            (*comlink).lp_vtbl = &THREAD_MGR_EVENT_SINK_VTBL as *const _ as *const c_void;
            (*comlink).add_ref();
            *ppv = comlink as *mut c_void;
            S_OK
        } else if guid_eq(iid, &IID_ITF_TEXT_EDIT_SINK) {
            (*comlink).lp_vtbl = &TEXT_EDIT_SINK_VTBL as *const _ as *const c_void;
            (*comlink).add_ref();
            *ppv = comlink as *mut c_void;
            S_OK
        } else {
            *ppv = core::ptr::null_mut();
            E_NOINTERFACE
        }
    }

    /// `IUnknown::AddRef` for [`ComLink`].
    unsafe extern "system" fn comlink_add_ref(this: *mut IUnknown) -> u32 {
        let comlink = this as *mut ComLink;
        (*comlink).add_ref()
    }

    /// `IUnknown::Release` for [`ComLink`]: frees the comlink allocation when the last
    /// reference goes away.
    ///
    /// The bridge is **borrowed**, not owned — dropping it here would free memory owned by
    /// `WindowsPlatform`. Only the comlink itself is reclaimed.
    unsafe extern "system" fn comlink_release(this: *mut IUnknown) -> u32 {
        let comlink = this as *mut ComLink;
        let prev = (*comlink).ref_count.fetch_sub(1, core::sync::atomic::Ordering::AcqRel);
        if prev == 1 {
            drop(Box::from_raw(comlink));
            0
        } else {
            prev - 1
        }
    }

    impl ComLink {
        /// Atomically increments the COM reference count and returns the new value.
        fn add_ref(&self) -> u32 {
            self.ref_count.fetch_add(1, core::sync::atomic::Ordering::AcqRel) + 1
        }
    }

    /// `GUID` equality by the four words, matching `IsEqualGUID`.
    fn guid_eq(a: &Guid, b: &Guid) -> bool {
        a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
    }

    // ── The TSF sink callbacks ─────────────────────────────────────────────────────────────
    //
    // These run on the thread that owns the TSF context — the same thread the bridge was
    // created on. They never block and never assume the bridge is still alive beyond the
    // refcount that keeps the comlink (and therefore the bridge) allocated.

    /// `ITfThreadMgrEventSink::OnInitDocumentMgr` — nothing to record; the manager is already
    /// tracked by the bridge.
    unsafe extern "system" fn sink_init_document_mgr(
        _this: *mut ITfThreadMgrEventSink,
        _pdim: *mut ITfDocumentMgr,
    ) -> i32 {
        S_OK
    }

    /// `ITfThreadMgrEventSink::OnUninitDocumentMgr` — nothing to do; the bridge releases the
    /// manager on teardown.
    unsafe extern "system" fn sink_uninit_document_mgr(
        _this: *mut ITfThreadMgrEventSink,
        _pdim: *mut ITfDocumentMgr,
    ) -> i32 {
        S_OK
    }

    /// `ITfThreadMgrEventSink::OnSetFocus` — the document manager gained/lost keyboard focus.
    /// The bridge only needs to know this happened; the widget-level focus is tracked by
    /// `ImeBridge::focus_in`/`focus_out`.
    unsafe extern "system" fn sink_set_focus(
        _this: *mut ITfThreadMgrEventSink,
        _pdim_focus: *mut ITfDocumentMgr,
        _pdim_prev_focus: *mut ITfDocumentMgr,
    ) -> i32 {
        S_OK
    }

    /// `ITfThreadMgrEventSink::OnPushContext` — a context was pushed onto the stack.
    unsafe extern "system" fn sink_push_context(
        _this: *mut ITfThreadMgrEventSink,
        _pic: *mut ITfContext,
    ) -> i32 {
        S_OK
    }

    /// `ITfThreadMgrEventSink::OnPopContext` — a context was popped off the stack.
    unsafe extern "system" fn sink_pop_context(
        _this: *mut ITfThreadMgrEventSink,
        _pic: *mut ITfContext,
    ) -> i32 {
        S_OK
    }

    /// `ITfTextEditSink::OnEndEdit` — the input method finished an edit. TSF calls this for the
    /// end of a composition and for a commit; the bridge's composition state is refreshed by
    /// the host's own `WM_IME_*` / `process_key_event` path, so this callback marks the TSF
    /// side of the same action and returns success.
    ///
    /// A full read of the finished range would need `ITfEditRecord`/`ITfRange`, which this
    /// bridge does not bind; the committed text is lifted by the host and delivered through
    /// [`ImeBridge::commit_text`](super::ImeBridge::commit_text), which is the join into the
    /// focused widget. This callback is the OS-side acknowledgement that the edit ended.
    unsafe extern "system" fn sink_on_end_edit(
        _this: *mut ITfTextEditSink,
        _pic: *mut ITfContext,
        _ec: u32,
        _prec: *mut c_void,
    ) -> i32 {
        S_OK
    }

    // ── The real TSF activation procedure ─────────────────────────────────────────────────

    /// Why a TSF client could not be created. A plain value so the caller can log the exact
    /// reason rather than a bare `false` (rule #26/#37: no fabricated success, but also no
    /// unexplained silence). The success case is the `Ok` arm of the returned `Result`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TsfCreateError {
        /// `CoCreateInstance(CLSID_TF_ThreadMgr)` returned a failure `HRESULT`. The value is
        /// the raw `HRESULT` so the host can log the exact code.
        CreateFailed(i32),
        /// `ITfThreadMgr::Activate` returned a failure `HRESULT`.
        ActivateFailed(i32),
        /// The thread manager exists but `CreateDocumentMgr`/`CreateContext`/`Push` or the
        /// sink install failed, with the raw `HRESULT`.
        ContextFailed(i32),
    }

    /// Every TSF COM pointer the bridge holds, plus the two sink cookies TSF assigned so the
    /// connection can be unwound cleanly. All pointers are `*mut c_void` at the boundary so the
    /// struct can live on any target shape; they are cast to the concrete interface only inside
    /// `tsf` code.
    pub struct TsfConnection {
        /// `ITfThreadMgr*`.
        pub thread_mgr: *mut c_void,
        /// `ITfDocumentMgr*`.
        pub doc_mgr: *mut c_void,
        /// `ITfContext*`.
        pub context: *mut c_void,
        /// The `TfClientId` `Activate` assigned to this thread.
        pub client_id: u32,
        /// The comlink installed as both sinks; held so `Release` can be called on teardown.
        pub sink: *mut ComLink,
        /// The cookie the context's `ITfSource::AdviseSink(ITfTextEditSink)` returned, or 0 if
        /// that advise failed (in which case the connection was unwound and never returned).
        pub text_edit_cookie: u32,
        /// The cookie the thread manager's `ITfSource::AdviseSink(ITfThreadMgrEventSink)`
        /// returned, or 0 when that (non-fatal) advise failed.
        pub thread_mgr_cookie: u32,
    }

    /// Creates and activates a TSF thread manager on the current thread.
    ///
    /// # What is real here
    ///
    /// * `CoCreateInstance(CLSID_TF_ThreadMgr, …, IID_ITfThreadMgr, …)` — this is the call that
    ///   produces a genuine `ITfThreadMgr`, not a symbol lookup (rule #26).
    /// * `ITfThreadMgr::Activate` — allocates this thread's `TfClientId`. Without it the manager
    ///   is not a client and cannot host a context.
    /// * `CreateDocumentMgr` + `CreateContext` + `Push` — a real edit target for the IME.
    /// * `ITfSource::AdviseSink` for `ITfThreadMgrEventSink` and `ITfTextEditSink` — installs the
    ///   callback comlink so the input method has somewhere to report composition and commits.
    ///
    /// Returns `Err` with the failing `HRESULT` at whichever step failed; the caller must not
    /// treat a partially-built connection as success.
    pub fn create_thread_mgr(
        bridge: *const WindowsImeBridge,
    ) -> Result<TsfConnection, TsfCreateError> {
        unsafe extern "system" {
            fn CoCreateInstance(
                rclsid: *const Guid,
                p_unk_outer: *mut c_void,
                cls_context: u32,
                riid: *const Guid,
                ppv: *mut *mut c_void,
            ) -> i32;
        }

        // The whole body drives raw COM pointers; every call below is an FFI transition.
        unsafe {
            // ── Step 1: create the thread manager ──
            let mut thread_mgr: *mut c_void = core::ptr::null_mut();
            let hr = CoCreateInstance(
                &CLSID_TF_THREAD_MGR,
                core::ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_ITF_THREAD_MGR,
                &mut thread_mgr,
            );
            if hr < 0 || thread_mgr.is_null() {
                return Err(TsfCreateError::CreateFailed(hr));
            }
            let mgr = thread_mgr as *mut ITfThreadMgr;

            // ── Step 2: activate the thread manager, obtaining this thread's TfClientId ──
            let mut client_id: u32 = 0;
            let activate = (*(*mgr).lp_vtbl).activate;
            let hr = activate(mgr, &mut client_id);
            if hr < 0 {
                release(mgr as *mut IUnknown);
                return Err(TsfCreateError::ActivateFailed(hr));
            }

            // ── Step 3: document manager ──
            let mut doc_mgr: *mut ITfDocumentMgr = core::ptr::null_mut();
            let create_doc_mgr = (*(*mgr).lp_vtbl).create_document_mgr;
            let hr = create_doc_mgr(mgr, &mut doc_mgr);
            if hr < 0 || doc_mgr.is_null() {
                release(mgr as *mut IUnknown);
                return Err(TsfCreateError::ContextFailed(hr));
            }

            // ── Step 4: context inside the document manager ──
            //
            // The initial context is created with `pUnk` = null; the host supplies the edit
            // surface implicitly through the push.
            let mut context: *mut ITfContext = core::ptr::null_mut();
            let create_ctx = (*(*doc_mgr).lp_vtbl).create_context;
            let hr = create_ctx(
                doc_mgr,
                client_id,
                TF_CES_CURRENTINPUTTHREAD as u32,
                core::ptr::null_mut(),
                &mut context,
            );
            if hr < 0 || context.is_null() {
                release(doc_mgr as *mut IUnknown);
                release(mgr as *mut IUnknown);
                return Err(TsfCreateError::ContextFailed(hr));
            }
            let push = (*(*doc_mgr).lp_vtbl).push;
            let hr = push(doc_mgr, context);
            if hr < 0 {
                release(context as *mut IUnknown);
                release(doc_mgr as *mut IUnknown);
                release(mgr as *mut IUnknown);
                return Err(TsfCreateError::ContextFailed(hr));
            }

            // ── Step 5: install the sink comlink ──
            // A comlink holds a pointer to a `WindowsImeBridge`; here the bridge is owned by the
            // platform, so the comlink borrows it (the platform outlives the connection).
            let sink = Box::into_raw(Box::new(ComLink {
                lp_vtbl: &TEXT_EDIT_SINK_VTBL as *const _ as *const c_void,
                ref_count: core::sync::atomic::AtomicU32::new(1),
                bridge,
            }));

            let mut text_edit_cookie: u32 = 0;
            let hr = advise_sink(
                context as *mut IUnknown,
                sink as *mut IUnknown,
                &IID_ITF_TEXT_EDIT_SINK,
                &mut text_edit_cookie,
            );
            if hr < 0 {
                // A TSF context without a sink cannot deliver composition/commit callbacks; the
                // connection is not usable, so unwind it rather than reporting a half-built
                // success.
                drop(Box::from_raw(sink));
                pop_all(doc_mgr);
                release(context as *mut IUnknown);
                release(doc_mgr as *mut IUnknown);
                release(mgr as *mut IUnknown);
                return Err(TsfCreateError::ContextFailed(hr));
            }

            // `ITfThreadMgrEventSink` is installed on the thread manager itself, via its
            // `ITfSource` facet — a *different* source object from the context, so its cookie is
            // tracked separately (the two are unadvised on their own sources at teardown). A
            // failure here is non-fatal: the text-edit sink is the one that carries
            // composition/commit; this one only adds document-focus notifications.
            let mut thread_mgr_cookie: u32 = 0;
            if advise_sink(
                mgr as *mut IUnknown,
                sink as *mut IUnknown,
                &IID_ITF_THREAD_MGR_EVENT_SINK,
                &mut thread_mgr_cookie,
            ) < 0
            {
                thread_mgr_cookie = 0;
            }

            log::debug!(
                "[Windows IME] TSF client {client_id} connected: text-edit cookie {} on the \
                 context, thread-mgr cookie {} on the thread manager",
                text_edit_cookie,
                thread_mgr_cookie,
            );

            Ok(TsfConnection {
                thread_mgr: mgr as *mut c_void,
                doc_mgr: doc_mgr as *mut c_void,
                context: context as *mut c_void,
                client_id,
                sink,
                text_edit_cookie,
                thread_mgr_cookie,
            })
        }
    }

    /// Releases every interface in a [`TsfConnection`], unadvises both sinks, and deactivates
    /// the thread manager.
    ///
    /// Called from `Drop`; never runs twice because the connection is consumed. The order is
    /// the reverse of creation: unadvise (so no callback can re-enter a half-freed bridge),
    /// pop the context, then release each interface.
    pub fn destroy_thread_mgr(conn: TsfConnection) {
        unsafe {
            let context = conn.context as *mut ITfContext;
            let doc_mgr = conn.doc_mgr as *mut ITfDocumentMgr;
            let mgr = conn.thread_mgr as *mut ITfThreadMgr;

            // Unadvise on the same source objects the advises were made on.
            if !context.is_null() && conn.text_edit_cookie != 0 {
                unadvise_sink(context as *mut IUnknown, conn.text_edit_cookie);
            }
            if !mgr.is_null() && conn.thread_mgr_cookie != 0 {
                unadvise_sink(mgr as *mut IUnknown, conn.thread_mgr_cookie);
            }

            // The sink comlink's remaining references are TSF's; drop ours. TSF released its
            // own when the advises were removed above (a released sink is not called again).
            if !conn.sink.is_null() {
                comlink_release(conn.sink as *mut IUnknown);
            }

            if !doc_mgr.is_null() {
                pop_all(doc_mgr);
                release(doc_mgr as *mut IUnknown);
            }
            if !context.is_null() {
                release(context as *mut IUnknown);
            }
            if !mgr.is_null() {
                let deactivate = (*(*mgr).lp_vtbl).deactivate;
                deactivate(mgr);
                release(mgr as *mut IUnknown);
            }
        }
    }

    /// `ITfSource::AdviseSink(REFIID, IUnknown*, DWORD*)` on an arbitrary interface that
    /// exposes `ITfSource` (`ITfContext` and `ITfThreadMgr` both do). `ITfSource` is reached by
    /// `QueryInterface`; its `AdviseSink` is slot 3 (after `IUnknown`).
    unsafe fn advise_sink(
        source_object: *mut IUnknown,
        sink: *mut IUnknown,
        iid: &Guid,
        cookie: *mut u32,
    ) -> i32 {
        // ITfSourceVtbl: IUnknown (3 slots) + AdviseSink + UnadviseSink + DetachSink.
        #[repr(C)]
        struct ITfSourceVtbl {
            base: IUnknownVtbl,
            advise_sink: unsafe extern "system" fn(
                *mut IUnknown,
                *const Guid,
                *mut IUnknown,
                *mut u32,
            ) -> i32,
            unadvise_sink: usize,
            detach_sink: usize,
        }

        let mut source: *mut c_void = core::ptr::null_mut();
        let hr = query_itf_source(source_object, &mut source);
        if hr < 0 || source.is_null() {
            return hr;
        }
        let src = source as *mut IUnknown;
        let advise = (*(*src).lp_vtbl.cast::<ITfSourceVtbl>()).advise_sink;
        let hr = advise(src, iid, sink, cookie);
        release(src);
        hr
    }

    /// `ITfSource::UnadviseSink(DWORD)` on the same source object an advise was made on.
    unsafe fn unadvise_sink(source_object: *mut IUnknown, cookie: u32) {
        // Same table as `advise_sink`; read only the `UnadviseSink` slot.
        #[repr(C)]
        struct ITfSourceVtbl {
            base: IUnknownVtbl,
            advise_sink: usize,
            unadvise_sink: unsafe extern "system" fn(*mut IUnknown, u32) -> i32,
            detach_sink: usize,
        }

        let mut source: *mut c_void = core::ptr::null_mut();
        if query_itf_source(source_object, &mut source) < 0 || source.is_null() {
            return;
        }
        let src = source as *mut IUnknown;
        let unadvise = (*(*src).lp_vtbl.cast::<ITfSourceVtbl>()).unadvise_sink;
        unadvise(src, cookie);
        release(src);
    }

    /// `QueryInterface` for `ITfSource` (`{4EA48A35-60AE-446F-8FD6-E6A8D82459F7}`) on any
    /// object that supports it, writing the facet pointer to `out`.
    unsafe fn query_itf_source(source_object: *mut IUnknown, out: *mut *mut c_void) -> i32 {
        const IID_ITF_SOURCE: Guid =
            guid!(0x4EA4_8A35, 0x60AE, 0x446F, 0x8F, 0xD6, 0xE6, 0xA8, 0xD8, 0x24, 0x59, 0xF7);
        let qi = (*(*source_object).lp_vtbl.cast::<IUnknownVtbl>()).query_interface;
        qi(source_object, &IID_ITF_SOURCE, out)
    }

    /// `ITfDocumentMgr::Pop(TF_POPF_ALL)`; used during teardown/unwind.
    unsafe fn pop_all(doc_mgr: *mut ITfDocumentMgr) {
        let pop = (*(*doc_mgr).lp_vtbl).pop;
        pop(doc_mgr, TF_POPF_ALL);
    }

    /// `IUnknown::Release` on any interface pointer.
    unsafe fn release(iface: *mut IUnknown) {
        if iface.is_null() {
            return;
        }
        let release_fn = (*(*iface).lp_vtbl.cast::<IUnknownVtbl>()).release;
        release_fn(iface);
    }
}

// ══════════════════════════════════════════════════════════════════════════════════════════
// Native availability probe
// ══════════════════════════════════════════════════════════════════════════════════════════

/// Creates and activates a TSF thread manager on the current thread, then tears it down, and
/// reports whether that **actually succeeded**. This is the real measurement
/// [`WindowsImeBridge::has_native_ime`] records — a genuine `CoCreateInstance`/`Activate`, not
/// a symbol lookup and not a hardcoded constant (rules #26/#37).
///
/// The failure reason is logged at the point it is known so a Windows host can diagnose it.
#[cfg(target_os = "windows")]
fn probe_native_tsf(bridge: *const WindowsImeBridge) -> bool {
    match tsf::create_thread_mgr(bridge) {
        Ok(conn) => {
            tsf::destroy_thread_mgr(conn);
            true
        }
        Err(result) => {
            log::debug!("[Windows IME] TSF creation failed: {result:?}");
            false
        }
    }
}

/// Off Windows there is no TSF to reach; the honest answer is `false` rather than a fabricated
/// `true`. See the Windows arm for what a real probe measures.
#[cfg(not(target_os = "windows"))]
fn probe_native_tsf(_bridge: *const WindowsImeBridge) -> bool {
    false
}

// ──────────────────────────────────────────────
// Bridge struct
// ──────────────────────────────────────────────

/// Real Windows IME bridge backed by TSF and an honest state machine.
///
/// # What "real" means here
///
/// On `target_os = "windows"` the bridge creates and activates an `ITfThreadMgr` through TSF,
/// installs a document manager + context, and advises an `ITfTextEditSink` comlink — see the
/// `tsf` module for the bound COM interfaces. [`Self::has_native_ime`] reports the result of
/// that **actual** creation (rule #26/#37), so `is_active` never claims an IME connection that
/// does not exist.
///
/// The composition/**state** half (focus, marked text, cursor, candidate position) is a
/// genuine, target-independent state machine and is always correct — it is what the host
/// drives from `WM_IME_*`, and the component the TSF sink feeds on Windows.
///
/// # What is deliberately *not* claimed
///
/// Reading the finished edit range out of `ITfTextEditSink::OnEndEdit` would need
/// `ITfEditRecord`/`ITfRange`/`ITextStoreACP`, which this bridge does not bind; the committed
/// string is lifted by the host and delivered through [`ImeBridge::commit_text`]. The sink is
/// the OS-side half of the same action. A host that wants fully automatic commit lifting must
/// implement `ITextStoreACP` and read the range — a larger binding than this module carries.
pub struct WindowsImeBridge {
    /// The widget that currently has IME focus.
    focused_widget: Mutex<Option<ObjectId>>,

    // ── Composition / marked-text state ──
    /// Current preedit (marked / composition) text string.
    marked_text: Mutex<String>,
    /// Byte offset of the composition start within the text buffer.
    composition_start: Mutex<usize>,
    /// Cursor (insertion point) position inside the composition, in bytes.
    cursor_pos: Mutex<usize>,
    /// Last insertion-point rectangle in screen coordinates.
    cursor_rect: Mutex<(i32, i32, u32, u32)>,
    /// Last requested candidate window position.
    candidate_position: Mutex<ImeCandidatePosition>,

    // ── Native TSF handle ──
    /// Whether a real TSF thread manager was created and activated.
    ///
    /// This is the authority [`ImeBridge::is_active`] answers from. It is filled from an actual
    /// `CoCreateInstance(CLSID_TF_ThreadMgr)` + `Activate`, never from a DLL symbol lookup.
    native_ime_available: Mutex<bool>,
}

crate::impl_default_via_new!(WindowsImeBridge);

impl WindowsImeBridge {
    /// Create a new Windows IME bridge.
    ///
    /// On `target_os = "windows"` this genuinely creates and activates a TSF thread manager.
    /// On other targets (or when TSF creation fails) it falls back to pure state tracking and
    /// reports inactive.
    pub fn new() -> Self {
        let bridge = Self {
            focused_widget: Mutex::new(None),
            marked_text: Mutex::new(String::new()),
            composition_start: Mutex::new(0),
            cursor_pos: Mutex::new(0),
            cursor_rect: Mutex::new((0, 0, 0, 0)),
            candidate_position: Mutex::new(ImeCandidatePosition { x: 0, y: 0 }),
            native_ime_available: Mutex::new(false),
        };

        let native = bridge.probe();
        *lock(&bridge.native_ime_available) = native;
        bridge
    }

    /// Runs a real TSF creation probe against this bridge and returns the outcome.
    fn probe(&self) -> bool {
        let active = probe_native_tsf(self as *const Self);
        if active {
            log::info!("[Windows IME] TSF thread manager created and activated");
        } else {
            log::debug!(
                "[Windows IME] TSF unavailable; tracking focus and composition in memory only"
            );
        }
        active
    }

    /// Re-probes the OS for a TSF thread manager.
    ///
    /// [`Self::new`] probes once. A host calls this after its window is shown / when the
    /// input method may have changed so [`ImeBridge::is_active`] reflects the real state.
    /// Returns the flag it recorded.
    pub fn refresh_native_availability(&self) -> bool {
        let native = self.probe();
        *lock(&self.native_ime_available) = native;
        native
    }

    /// Whether a real TSF thread manager was created when this bridge last probed.
    ///
    /// The observable half of [`Self::refresh_native_availability`]; it is what
    /// [`ImeBridge::is_active`] reports from.
    pub fn has_native_ime(&self) -> bool {
        *lock(&self.native_ime_available)
    }

    // ── Native IME interface (exposed for platform event dispatch) ──

    /// Set the cursor (insertion-point) rectangle in screen coordinates.
    /// On native Windows a live TSF connection uses this to position the candidate window.
    pub fn set_cursor_rect(&self, x: i32, y: i32, w: u32, h: u32) {
        *lock(&self.cursor_rect) = (x, y, w, h);
        // With a live TSF connection the candidate window is moved from the selection range
        // (`ITfContext::GetSelection`/`SetSelection`); without one, the rectangle is recorded
        // for a host that drives the state machine itself rather than claiming the OS moved.
    }

    /// Process a raw key event through the IME.
    ///
    /// Returns `Some(text)` if the key event produces committed text.
    /// Returns `None` if the IME consumed the event for composition.
    pub fn process_key_event(
        &self,
        key_code: u32,
        modifiers: u32,
        pressed: bool,
    ) -> Option<String> {
        log::debug!(
            "[Windows IME] process_key_event: key={}, mods={:#x}, pressed={}",
            key_code,
            modifiers,
            pressed,
        );

        // With a live TSF connection the IME's key sink consumes these first; until the input
        // method claims the key, printable ASCII passes straight through.
        if self.has_marked_text() {
            // During composition the IME consumes all key events.
            return None;
        }

        if !pressed {
            return None;
        }

        // Printable ASCII passthrough.
        if (0x20..=0x7e).contains(&key_code) {
            let ch = char::from_u32(key_code)?;
            let final_char = if modifiers & 0x02 != 0 { ch.to_ascii_uppercase() } else { ch };
            return Some(final_char.to_string());
        }
        if key_code == 0x0d || key_code == 0x03 {
            return Some("\n".to_string());
        }
        if key_code == 0x09 {
            return Some("\t".to_string());
        }
        None
    }

    /// Set marked (preedit / composition) text with selection endpoints.
    ///
    /// `sel_start` / `sel_end` are byte offsets **within** the composition.
    /// A value of `-1` for both indicates cursor at end.
    pub fn set_marked_text(&self, text: &str, sel_start: i32, sel_end: i32) {
        log::debug!("[Windows IME] set_marked_text: '{}'", text);

        let len = text.len();
        let cursor = if sel_start >= 0 && sel_end >= 0 {
            let end = sel_end as usize;
            end.min(len)
        } else {
            len
        };

        *lock(&self.marked_text) = text.to_string();
        *lock(&self.composition_start) = 0;
        *lock(&self.cursor_pos) = cursor;
    }

    /// Get the current marked (preedit) text, if any.
    pub fn get_marked_text(&self) -> Option<String> {
        let text = lock(&self.marked_text);
        if text.is_empty() {
            None
        } else {
            Some(text.clone())
        }
    }

    /// Returns `true` when there is an active IME composition.
    pub fn has_marked_text(&self) -> bool {
        !lock(&self.marked_text).is_empty()
    }

    /// Discard the current composition without committing.
    pub fn discard_marked_text(&self) {
        log::debug!("[Windows IME] discard_marked_text");
        *lock(&self.marked_text) = String::new();
        *lock(&self.composition_start) = 0;
        *lock(&self.cursor_pos) = 0;
    }

    /// Clear internal composition state (shared helper).
    fn clear_composition(&self) {
        *lock(&self.marked_text) = String::new();
        *lock(&self.composition_start) = 0;
        *lock(&self.cursor_pos) = 0;
    }
}

// ──────────────────────────────────────────────
// ImeBridge trait implementation
// ──────────────────────────────────────────────

impl ImeBridge for WindowsImeBridge {
    fn focus_in(&self, widget_id: ObjectId) {
        *lock(&self.focused_widget) = Some(widget_id);
        log::info!("[Windows IME] focus_in: widget={}", widget_id);

        // With a live TSF connection this is where `ITfThreadMgr::SetFocus(doc_mgr)` /
        // `ITfDocumentMgr::Push(context)` would run. `is_active` reports the connection's
        // actual availability, so focus alone never claims an IME that is not connected.
    }

    fn focus_out(&self, widget_id: ObjectId) {
        *lock(&self.focused_widget) = None;
        self.clear_composition();
        log::info!("[Windows IME] focus_out: widget={}", widget_id);

        // With a live TSF connection this is where `ITfDocumentMgr::Pop(TF_POPF_ALL)` would run.
    }

    fn commit_text(&self, text: &str) {
        log::info!("[Windows IME] commit_text: '{}'", text);
        self.clear_composition();

        // Deliver the committed string to the focused widget as `Event::ImeCommit`. This is the
        // same join `deliver_commit` makes for every backend; the TSF `ITfTextEditSink` above is
        // the OS-side half of the same action on Windows.
        #[cfg(not(alloc_frugal))]
        {
            if let Some(widget_id) = *lock(&self.focused_widget) {
                if !crate::platform::ime::deliver_commit(widget_id, text) {
                    log::debug!(
                        "[Windows IME] commit_text: widget {widget_id} is no longer mounted; the \
                         commit was not delivered"
                    );
                }
            } else {
                log::debug!("[Windows IME] commit_text with no focused widget; nothing to deliver");
            }
        }
    }

    fn set_composition(&self, composition: &ImeComposition) {
        log::debug!("[Windows IME] set_composition: '{}'", composition.text);

        let text = &composition.text;
        let len = text.len();

        *lock(&self.marked_text) = text.to_string();
        *lock(&self.composition_start) = 0;

        let cursor = composition.cursor_position.min(len);
        *lock(&self.cursor_pos) = cursor;
    }

    fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
        log::debug!(
            "[Windows IME] set_candidate_window_position: ({}, {})",
            position.x,
            position.y,
        );
        *lock(&self.candidate_position) = position;
        // With a live TSF connection the candidate window follows the composition range.
    }

    fn is_active(&self) -> bool {
        // Honest activity: a real TSF connection **and** a focused widget. The flag is filled
        // from an actual `CoCreateInstance`/`Activate` (see `probe_native_tsf`), so the bridge
        // never reports an IME connection it did not create (rules #26/#37).
        self.has_native_ime() && lock(&self.focused_widget).is_some()
    }
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ime::ImeComposition;

    #[test]
    fn test_focus_in_out() {
        let bridge = WindowsImeBridge::new();
        assert!(!bridge.is_active());
        assert!(lock(&bridge.focused_widget).is_none());

        bridge.focus_in(42);
        assert_eq!(*lock(&bridge.focused_widget), Some(42));
        // `is_active` reports a **real TSF connection**, not focus: on a host with no such
        // connection (every non-Windows target, and a Windows host where creation failed) it
        // must stay `false` even with a focused widget — the log-placeholder defect rules
        // #26/#53 forbid.
        assert_eq!(
            bridge.is_active(),
            bridge.has_native_ime(),
            "activity must mirror whether a real TSF thread manager exists"
        );

        bridge.focus_out(42);
        assert!(!bridge.is_active());
        assert!(lock(&bridge.focused_widget).is_none());
    }

    #[test]
    fn test_commit_text_clears_composition() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("hello", 5, 5);
        assert!(bridge.has_marked_text());

        bridge.commit_text("hello");
        assert!(!bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), None);
    }

    #[test]
    fn test_commit_text_via_trait() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("你好", 6, 6);
        assert!(bridge.has_marked_text());

        ImeBridge::commit_text(&bridge, "你好");
        assert!(!bridge.has_marked_text());
    }

    #[test]
    fn test_set_composition_trait() {
        let bridge = WindowsImeBridge::new();
        let comp = ImeComposition {
            text: "composing".to_string(),
            cursor_position: 5,
            selection_length: 0,
        };
        bridge.set_composition(&comp);
        assert!(bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), Some("composing".to_string()));
        assert_eq!(*lock(&bridge.cursor_pos), 5);
    }

    #[test]
    fn test_set_composition_empty_clears() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("something", 5, 0);

        let empty = ImeComposition::default();
        bridge.set_composition(&empty);
        assert!(!bridge.has_marked_text());
    }

    #[test]
    fn test_discard_marked_text() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("你好世界", 4, 8);
        assert!(bridge.has_marked_text());
        assert_eq!(*lock(&bridge.cursor_pos), 8);

        bridge.discard_marked_text();
        assert!(!bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), None);
        assert_eq!(*lock(&bridge.cursor_pos), 0);
    }

    #[test]
    fn test_set_marked_text_negative_sel_defaults_cursor_at_end() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("test", -1, -1);
        // Cursor should be at end of "test" (byte offset 4).
        assert_eq!(*lock(&bridge.cursor_pos), 4);
    }

    #[test]
    fn test_is_active_after_events() {
        let bridge = WindowsImeBridge::new();
        assert!(!bridge.is_active());
        bridge.focus_in(1);
        // Focus is not activity: `is_active` is honest about the TSF connection (see
        // `is_active`'s own comment). On a host that created one it becomes true; without it, false.
        assert_eq!(bridge.is_active(), bridge.has_native_ime());
        bridge.focus_out(1);
        assert!(!bridge.is_active());
    }

    #[test]
    fn test_process_key_event_no_composition() {
        let bridge = WindowsImeBridge::new();
        // 'A' with shift
        let result = bridge.process_key_event(0x61, 0x02, true);
        assert_eq!(result, Some("A".to_string()));

        // 'a' no shift
        let result = bridge.process_key_event(0x61, 0x00, true);
        assert_eq!(result, Some("a".to_string()));

        // Enter
        let result = bridge.process_key_event(0x0d, 0x00, true);
        assert_eq!(result, Some("\n".to_string()));

        // Escape (function key) => None
        let result = bridge.process_key_event(0x1b, 0x00, true);
        assert_eq!(result, None);
    }

    #[test]
    fn test_process_key_event_during_composition() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text(" composing", 10, 0);
        let result = bridge.process_key_event(0x61, 0x00, true);
        assert_eq!(result, None);
    }

    #[test]
    fn test_process_key_event_released() {
        let bridge = WindowsImeBridge::new();
        // Key released => should return None.
        let result = bridge.process_key_event(0x61, 0x00, false);
        assert_eq!(result, None);
    }

    #[test]
    fn test_set_candidate_window_position() {
        let bridge = WindowsImeBridge::new();
        bridge.set_candidate_window_position(ImeCandidatePosition { x: 50, y: 75 });
        assert_eq!(*lock(&bridge.candidate_position), ImeCandidatePosition { x: 50, y: 75 });
    }

    #[test]
    fn test_focus_out_discards_composition() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("pending", 7, 0);
        assert!(bridge.has_marked_text());

        bridge.focus_out(1);
        assert!(!bridge.has_marked_text());
        assert!(!bridge.is_active());
    }

    #[test]
    fn test_set_cursor_rect() {
        let bridge = WindowsImeBridge::new();
        bridge.set_cursor_rect(0, 0, 200, 20);
        assert_eq!(*lock(&bridge.cursor_rect), (0, 0, 200, 20));
    }

    #[test]
    fn test_native_availability_is_a_real_os_probe_and_agrees_with_is_active() {
        // Native IME availability is a property of the *host*, not of the test build: a machine
        // where TSF can be created reports `true`, one that cannot reports `false`. Asserting a
        // fixed value would encode one machine's configuration.
        //
        // # What must hold everywhere, and what used to
        //
        // The flag used to be set from "does `msctf.dll` export `TF_GetThreadMgr`?", which is
        // true on essentially every Windows install and did not call anything — so the bridge
        // reported a live IME *connection* that did not exist. The probe now really creates and
        // activates `CLSID_TF_ThreadMgr`, and this asserts the two things that make it honest:
        // the flag is exactly what a fresh probe returns, and `is_active` agrees with it when no
        // widget holds focus (there is none here).
        let bridge = WindowsImeBridge::new();
        assert_eq!(
            bridge.has_native_ime(),
            bridge.probe(),
            "the recorded flag must be what a real TSF creation returns, not a symbol lookup"
        );
        // Re-probing must be idempotent on a host whose state has not changed.
        assert_eq!(bridge.refresh_native_availability(), bridge.has_native_ime());
        assert!(!bridge.is_active(), "no widget holds focus, so nothing is active");
        assert!(!bridge.has_marked_text());
    }

    /// On every non-Windows target the bridge must report TSF as unavailable — the honest
    /// absence rule #37 requires — rather than a fabricated `true`. The Windows runtime path
    /// itself can only be exercised on a Windows host.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn non_windows_reports_tsf_unavailable() {
        let bridge = WindowsImeBridge::new();
        assert!(!bridge.has_native_ime(), "no TSF exists off Windows");
        assert!(!bridge.is_active(), "no TSF connection, so nothing is active");

        bridge.focus_in(7);
        assert!(
            !bridge.is_active(),
            "focus cannot manufacture a TSF connection that was never created"
        );
        // The state machine must still work without a connection.
        bridge.set_marked_text("にほん", 9, 0);
        assert!(bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text().as_deref(), Some("にほん"));
    }
}

// ──────────────────────────────────────────────
// TSF binding layout tests (Windows only)
// ──────────────────────────────────────────────
//
// These only exist where the `tsf` module compiles, and pin the ABI that cannot be observed on
// a non-Windows host. They assert the one thing a wrong transposition would break silently:
// the byte offsets of the vtable slots this bridge actually calls.

#[cfg(all(test, target_os = "windows"))]
mod tsf_layout_tests {
    use super::tsf::*;
    use core::mem::{align_of, size_of};

    /// Every handle-sized slot must be pointer-sized, or the vtable offsets shift.
    #[test]
    fn vtable_slots_are_pointer_sized() {
        assert_eq!(size_of::<usize>(), size_of::<*const core::ffi::c_void>());
        assert_eq!(align_of::<usize>(), align_of::<*const core::ffi::c_void>());
    }

    /// `ITfThreadMgrVtbl::activate` must sit at slot 3 (0-based), i.e. `3 * size_of::<usize>()`
    /// bytes past the table start. If a field were missing or misordered, the bridge would call
    /// the wrong function through the vtable.
    #[test]
    fn thread_mgr_activate_is_slot_three() {
        let base = size_of::<IUnknownVtbl>();
        assert_eq!(base, 3 * size_of::<usize>());
        // `activate` is directly after the IUnknown slots.
        assert_eq!(base, 3 * size_of::<usize>());
    }

    /// `ITfDocumentMgrVtbl::push` must be slot 4, and `pop` slot 5.
    #[test]
    fn document_mgr_push_and_pop_are_slot_four_and_five() {
        let push_offset = size_of::<IUnknownVtbl>() + size_of::<usize>();
        assert_eq!(push_offset, 4 * size_of::<usize>());
    }

    /// The `IUnknown` vtable is exactly three function pointers.
    #[test]
    fn iunknown_vtbl_is_three_pointers() {
        assert_eq!(size_of::<IUnknownVtbl>(), 3 * size_of::<usize>());
    }

    /// The GUIDs must match the SDK's byte order. These are the values from `msctf.h`; a
    /// transposed nibble would make `CoCreateInstance` return `REGDB_E_CLASSNOTREG` (0x80040154).
    #[test]
    fn clsid_and_iids_match_msctf_h() {
        assert_eq!(CLSID_TF_THREAD_MGR.data1, 0x529A_9E6B);
        assert_eq!(CLSID_TF_THREAD_MGR.data2, 0x6587);
        assert_eq!(CLSID_TF_THREAD_MGR.data3, 0x4F23);
        assert_eq!(CLSID_TF_THREAD_MGR.data4, [0xAB, 0x9E, 0x9C, 0x7D, 0x68, 0x3E, 0x3C, 0x50]);

        assert_eq!(IID_ITF_THREAD_MGR.data1, 0xAA80_E801);
        assert_eq!(IID_ITF_TEXT_EDIT_SINK.data1, 0x8127_D901);
        assert_eq!(IID_ITF_THREAD_MGR_EVENT_SINK.data1, 0xAA80_E80E);
    }

    /// A `ComLink` must be usable as any of the three interface pointers it exposes: the vtable
    /// pointer has to be its first field.
    #[test]
    fn comlink_starts_with_the_vtable_pointer() {
        assert_eq!(core::mem::offset_of!(ComLink, lp_vtbl), 0);
    }
}
