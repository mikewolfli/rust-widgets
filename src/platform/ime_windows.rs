// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Windows IME bridge — a real TSF (Text Services Framework) probe plus an honest state machine.
//!
//! On `target_os = "windows"` this module binds the TSF COM interfaces by hand and asks the OS
//! the one question it can answer correctly: can this thread create and activate an
//! `ITfThreadMgr` (`CoCreateInstance(CLSID_TF_ThreadMgr)` + `Activate`)? That result is what
//! `WindowsImeBridge::has_native_ime` records. Building an edit context on top of the thread
//! manager is only defined against a real `ITextStoreACP` text store; there is none here, and
//! issuing those calls anyway faulted on Windows (see the `tsf` module), so the binding honestly
//! stops at activation.
//!
//! On every other target — and in headless testing — the same type is a pure state machine
//! that correctly tracks marked text, composition start offsets, and cursor positions, and
//! reports `ImeBridge::is_active` as `false` because there is genuinely no TSF connection.

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
//
// # Why the binding stops after `Activate`
//
// The probe's question is "can this thread create and activate a TSF thread manager?". The
// next steps — `CreateDocumentMgr`, `CreateContext`, `Push`, and advising an `ITfTextEditSink`
// comlink — build an *edit target*, which TSF only defines in terms of a real `ITextStoreACP`
// text store. There is no store in a capability probe, and issuing those calls anyway was
// measured to fault with `STATUS_ACCESS_VIOLATION` (`0xc0000005`). Rather than transcribe an
// edit-context setup it cannot complete, the binding stops at the boundary of what it can do
// correctly (rule #37): the used slots are exactly `Activate` / `Deactivate`.

#[cfg(target_os = "windows")]
mod tsf {
    use core::ffi::c_void;

    /// `S_OK`. Any `HRESULT >= 0` is success, but this is the only success these calls return.
    pub const S_OK: i32 = 0;

    /// `CLSCTX_INPROC_SERVER` (0x1): the TSF thread manager is an in-process COM server.
    pub const CLSCTX_INPROC_SERVER: u32 = 0x1;

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

    /// `ITfThreadMgrVtbl`, transcribed up to and including the last slot this bridge calls:
    /// `Activate` (0-based slot 3) and `Deactivate` (slot 4). The remaining ~20 methods are not
    /// called, so they are not transcribed — a slot that is never read is a transposition that
    /// could be silently wrong.
    #[repr(C)]
    pub struct ITfThreadMgrVtbl {
        /// `IUnknown` slots.
        pub base: IUnknownVtbl,
        /// `HRESULT Activate(TfClientId*)`.
        pub activate: unsafe extern "system" fn(*mut ITfThreadMgr, *mut u32) -> i32,
        /// `HRESULT Deactivate()`.
        pub deactivate: unsafe extern "system" fn(*mut ITfThreadMgr) -> i32,
    }

    // ── COM apartment staging for the probe ──

    /// `S_FALSE` — `CoInitializeEx` reporting the thread was already in the requested apartment.
    pub const S_FALSE: i32 = 1;

    /// `RPC_E_CHANGED_MODE` — the thread is already in the **multithreaded** apartment, so the STA
    /// TSF requires cannot be entered without tearing down COM for the whole thread.
    pub const RPC_E_CHANGED_MODE: i32 = 0x8001_0106u32 as i32;

    /// `COINIT_APARTMENTTHREADED`.
    pub const COINIT_APARTMENTTHREADED: u32 = 0x2;

    /// Why a TSF probe could not be staged on this thread.
    ///
    /// A plain value (like [`TsfCreateError`]) so the caller logs the exact reason instead of a bare
    /// `false`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ComStaError {
        /// The thread is already in the MTA; carrying on would create an `ITfThreadMgr` the thread
        /// cannot legally host, which is the state that used to hang then fault.
        Multithreaded(i32),
        /// `CoInitializeEx` failed for some other reason, with the raw `HRESULT`.
        Init(i32),
    }

    unsafe extern "system" {
        /// `HRESULT CoInitializeEx(void* pvReserved, DWORD dwCoInit)`.
        fn CoInitializeEx(pv_reserved: *mut c_void, dw_coinit: u32) -> i32;
        /// `void CoUninitialize()`. Reverses exactly one successful `CoInitializeEx` on this thread.
        fn CoUninitialize();
    }

    /// Holds the thread in the STA for as long as it lives, undoing **only** what this scope added.
    ///
    /// TSF creates a thread-affine `ITfThreadMgr` and documents an STA requirement, so the probe
    /// must run inside one. `CoInitializeEx` is reference-counted per thread and *does not* switch a
    /// thread that is already in the MTA (`RPC_E_CHANGED_MODE`), so the guard records whether
    /// **this** call did the initialisation and calls `CoUninitialize` only then — it never
    /// disturbs an apartment the host, or a previous test on a reused worker thread, established.
    pub struct ComStaGuard {
        /// Whether this scope owns one `CoInitializeEx` reference to release.
        owned: bool,
    }

    impl Drop for ComStaGuard {
        fn drop(&mut self) {
            if self.owned {
                // SAFETY: balances the `CoInitializeEx` that returned `S_OK` for this scope.
                unsafe { CoUninitialize() };
            }
        }
    }

    /// Enters the STA for the caller's scope, or reports why it cannot.
    pub fn enter_sta() -> Result<ComStaGuard, ComStaError> {
        // SAFETY: both calls are the documented COM apartment entry/exit for this thread, and the
        // apartment this scope changes is restored by `ComStaGuard::drop`.
        unsafe {
            let hr = CoInitializeEx(core::ptr::null_mut(), COINIT_APARTMENTTHREADED);
            if hr == S_OK {
                Ok(ComStaGuard { owned: true })
            } else if hr == S_FALSE {
                Ok(ComStaGuard { owned: false })
            } else if hr == RPC_E_CHANGED_MODE {
                Err(ComStaError::Multithreaded(hr))
            } else {
                Err(ComStaError::Init(hr))
            }
        }
    }

    // ── The real TSF activation procedure ─────────────────────────────────────────────────

    /// Why a TSF client could not be created. A plain value so the caller can log the exact
    /// reason rather than a bare `false` (rule #26/#37: no fabricated success, but also no
    /// unexplained silence). The success case is the `Ok` arm of the returned `Result`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TsfCreateError {
        /// `CoCreateInstance(CLSID_TF_ThreadMgr)` returned a failure `HRESULT`. The value is
        /// the raw `HRESULT` so the host can log the exact code.
        CoCreate(i32),
        /// `ITfThreadMgr::Activate` returned a failure `HRESULT`.
        Activate(i32),
    }

    /// A live TSF connection: the activated `ITfThreadMgr`. The pointer is `*mut c_void` at the
    /// boundary so the struct can live on any target shape; it is cast to the concrete interface
    /// only inside `tsf` code.
    pub struct TsfConnection {
        /// `ITfThreadMgr*`.
        pub thread_mgr: *mut c_void,
    }

    /// Creates and activates a TSF thread manager on the current thread.
    ///
    /// # What is real here
    ///
    /// * `CoCreateInstance(CLSID_TF_ThreadMgr, …, IID_ITfThreadMgr, …)` — this is the call that
    ///   produces a genuine `ITfThreadMgr`, not a symbol lookup (rule #26).
    /// * `ITfThreadMgr::Activate` — allocates this thread's `TfClientId`. Without it the manager
    ///   is not a client.
    ///
    /// # What is deliberately not attempted
    ///
    /// `CreateDocumentMgr`/`CreateContext`/`Push` and advising an edit sink build an edit target
    /// that is only defined against a real `ITextStoreACP` text store. There is none here, and
    /// issuing those calls anyway faulted on Windows; the binding stops at activation rather than
    /// transcribe a setup it cannot complete (rule #37).
    ///
    /// Returns `Err` with the failing `HRESULT` at whichever step failed; the caller must not
    /// treat a partially-built connection as success.
    pub fn create_thread_mgr() -> Result<TsfConnection, TsfCreateError> {
        unsafe extern "system" {
            fn CoCreateInstance(
                rclsid: *const Guid,
                p_unk_outer: *mut c_void,
                cls_context: u32,
                riid: *const Guid,
                ppv: *mut *mut c_void,
            ) -> i32;
        }

        // The body drives a raw COM pointer; every call below is an FFI transition.
// SAFETY: the body drives a COM interface created on this thread; every pointer is checked before use.
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
                return Err(TsfCreateError::CoCreate(hr));
            }
            let mgr = thread_mgr as *mut ITfThreadMgr;

            // ── Step 2: activate the thread manager, obtaining this thread's TfClientId ──
            let mut client_id: u32 = 0;
            let activate = (*(*mgr).lp_vtbl).activate;
            let hr = activate(mgr, &mut client_id);
            if hr < 0 {
                release(mgr as *mut IUnknown);
                return Err(TsfCreateError::Activate(hr));
            }

            log::debug!("[Windows IME] TSF client {client_id} created and activated");

            Ok(TsfConnection { thread_mgr: mgr as *mut c_void })
        }
    }

    /// Deactivates and releases the [`TsfConnection`]'s thread manager.
    ///
    /// Called from `Drop`; never runs twice because the connection is consumed.
    pub fn destroy_thread_mgr(conn: TsfConnection) {
// SAFETY: `conn` owns the COM pointer, is consumed here, and cannot be released twice.
        unsafe {
            let mgr = conn.thread_mgr as *mut ITfThreadMgr;
            if mgr.is_null() {
                return;
            }
            let deactivate = (*(*mgr).lp_vtbl).deactivate;
            deactivate(mgr);
            release(mgr as *mut IUnknown);
        }
    }

    /// `IUnknown::Release` on any interface pointer.
    ///
    /// # Safety
    /// `iface` must be a live COM interface owned by the caller, and this call consumes it.
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
/// # Why the probe stages an STA first
///
/// TSF creates a thread-affine `ITfThreadMgr` and requires an STA. Calling `CoCreateInstance` on a
/// thread left in the MTA does not fail cleanly: it hangs, then faults with
/// `STATUS_ACCESS_VIOLATION` when the interface is used. A reused test worker thread (or a host
/// thread some other component put in the MTA) is exactly that state, so the probe enters the STA
/// itself — or, if the thread is already MTA, reports the honest absence without touching TSF.
///
/// The failure reason is logged at the point it is known so a Windows host can diagnose it.
///
/// # Why the probe is serialised
///
/// TSF's thread manager is process-wide COM state. Two threads creating and destroying one at the
/// same time deadlocked the test binary even after each was in its own STA, so the create/teardown
/// pair runs under a process-global lock. It is held only for the probe's own few milliseconds.
#[cfg(target_os = "windows")]
static PROBE_LOCK: crate::compat::Mutex<()> = crate::compat::Mutex::new(());

#[cfg(target_os = "windows")]
fn probe_native_tsf() -> bool {
    let sta = match tsf::enter_sta() {
        Ok(guard) => guard,
        Err(reason) => {
            log::debug!("[Windows IME] TSF probe skipped: {reason:?}");
            return false;
        }
    };
    // Serialise the whole create/teardown. TSF's thread manager is process-wide state, and two
    // threads racing to create and destroy one at the same time deadlocked the test binary; a
    // single probe holds this lock for only the few milliseconds it runs.
    let _serialised = crate::compat::lock(&PROBE_LOCK);
    let active = match tsf::create_thread_mgr() {
        Ok(conn) => {
            tsf::destroy_thread_mgr(conn);
            true
        }
        Err(result) => {
            log::debug!("[Windows IME] TSF creation failed: {result:?}");
            false
        }
    };
    // Leave the thread's apartment as it was found before reporting.
    drop(sta);
    active
}

/// Off Windows there is no TSF to reach; the honest answer is `false` rather than a fabricated
/// `true`. See the Windows arm for what a real probe measures.
#[cfg(not(target_os = "windows"))]
fn probe_native_tsf() -> bool {
    false
}

// ──────────────────────────────────────────────
// Bridge struct
// ──────────────────────────────────────────────

/// Real Windows IME bridge backed by a TSF availability probe and an honest state machine.
///
/// # What "real" means here
///
/// On `target_os = "windows"` the bridge creates and activates a genuine `ITfThreadMgr` through
/// TSF — see the `tsf` module for the bound COM interface. [`Self::has_native_ime`] reports the
/// result of that **actual** creation (rule #26/#37), so `is_active` never claims an IME
/// connection that does not exist.
///
/// The composition/**state** half (focus, marked text, cursor, candidate position) is a
/// genuine, target-independent state machine and is always correct — it is what the host
/// drives from `WM_IME_*`.
///
/// # What is deliberately *not* claimed
///
/// An edit *context* (which is what would carry composition into the OS input method) is only
/// defined against a real `ITextStoreACP` text store. This bridge does not implement one, so it
/// does not attempt `CreateDocumentMgr`/`CreateContext`/`Push` — measured on Windows, issuing
/// those without a store faulted (`STATUS_ACCESS_VIOLATION`). Committed text is lifted by the
/// host and delivered through [`ImeBridge::commit_text`]. A host that wants the OS to write the
/// composition directly must implement `ITextStoreACP` and advise an `ITfTextEditSink` — a
/// larger binding than this module carries.
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

    /// Runs a real TSF creation probe and returns the outcome.
    fn probe(&self) -> bool {
        let active = probe_native_tsf();
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
        // With an edit context installed the candidate window is moved from the selection range
        // (via the TSF context API, which this module does not bind); without one, the rectangle
        // is recorded for a host that drives the state machine itself rather than claiming the
        // OS moved.
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

        // Installing an edit context here (`ITfThreadMgr::SetFocus` + a pushed context) is only
        // defined against a real `ITextStoreACP` store, which this bridge does not implement; the
        // binding stops at the activated thread manager. `is_active` reports the connection's
        // actual availability, so focus alone never claims an IME that is not connected.
    }

    fn focus_out(&self, widget_id: ObjectId) {
        *lock(&self.focused_widget) = None;
        self.clear_composition();
        log::info!("[Windows IME] focus_out: widget={}", widget_id);

        // Popping the edit context would happen here if one had been pushed; none is, because
        // this bridge binds only the activated thread manager.
    }

    fn commit_text(&self, text: &str) {
        log::info!("[Windows IME] commit_text: '{}'", text);
        self.clear_composition();

        // Deliver the committed string to the focused widget as `Event::ImeCommit`. This is the
        // same join `deliver_commit` makes for every backend. On Windows the committed string is
        // lifted by the host (from `WM_IME_*`/`process_key_event`) and handed to this method;
        // this bridge installs no TSF edit sink, so it does not read the range itself.
        #[cfg(not(alloc_frugal))]
        {
            // Copy the focused widget out of the lock and release the guard before
            // delivering: `deliver_commit` dispatches to the widget, whose handler may
            // change focus, cancel or re-submit — any of which re-enters this bridge's
            // own `focused_widget` mutex. Holding the guard across that call deadlocks.
            let focused = *lock(&self.focused_widget);
            if let Some(widget_id) = focused {
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

    /// A probe on a thread that is already in the **multithreaded** apartment must be refused, never
    /// crash the process.
    ///
    /// TSF creates a thread-affine `ITfThreadMgr` and requires an STA. Running the full lib suite
    /// left the (reused) test thread in the MTA — some earlier test had initialised COM that way —
    /// and the next probe dereferenced the interface pointer it got anyway and took the process down
    /// with `STATUS_ACCESS_VIOLATION` (`0xc0000005`). This reproduces that state deterministically:
    /// enter the MTA on a worker thread, then build a bridge on it.
    #[cfg(target_os = "windows")]
    #[test]
    fn a_probe_on_a_multithreaded_apartment_is_refused_instead_of_crashing() {
        unsafe extern "system" {
            fn CoInitializeEx(pv_reserved: *mut std::ffi::c_void, dw_coinit: u32) -> i32;
            fn CoUninitialize();
        }
        /// `COINIT_MULTITHREADED`.
        const COINIT_MULTITHREADED: u32 = 0x0;

        let (hr, available, reprobe) = std::thread::spawn(|| {
            // SAFETY: the documented COM apartment entry; paired with `CoUninitialize` below when
            // the call added a reference (`hr >= 0`).
            let hr = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED) };
            // Constructing the bridge is what probes TSF; this must not fault.
            let bridge = WindowsImeBridge::new();
            let available = bridge.has_native_ime();
            let reprobe = bridge.probe();
            if hr >= 0 {
                // SAFETY: reverses the `CoInitializeEx` above.
                unsafe { CoUninitialize() };
            }
            (hr, available, reprobe)
        })
        .join()
        .expect("the MTA probe thread must not crash the process");

        // `hr >= 0` means the thread really was in the MTA, so the refusal is what is being
        // asserted. `RPC_E_CHANGED_MODE` (a thread already in the STA) would mean this run did not
        // exercise the MTA path; the bridge is then legitimately free to report `true`.
        if hr >= 0 {
            assert!(
                !available,
                "an MTA thread cannot host TSF, so the probe must report the honest `false`"
            );
            assert!(!reprobe, "re-probing on the same MTA thread must agree");
        }
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

    /// A commit callback that re-enters the bridge (focus_out) must not deadlock.
    ///
    /// `commit_text` used to hold the `focused_widget` lock for the whole synchronous
    /// `deliver_commit`, so a widget whose `ImeCommit` handler changed focus re-locked
    /// the same mutex and hung. The lock must be released before delivery.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn commit_text_releases_the_focus_lock_before_delivery() {
        use crate::event::{Event, EventHandler};
        use std::rc::Rc;

        let bridge = Rc::new(WindowsImeBridge::new());
        let reentered = Rc::new(core::cell::Cell::new(false));
        let id_cell = Rc::new(core::cell::Cell::new(0u64));

        struct Reentrant {
            bridge: Rc<WindowsImeBridge>,
            id: Rc<core::cell::Cell<u64>>,
            reentered: Rc<core::cell::Cell<bool>>,
            base: crate::widget::BaseWidget,
        }
        impl crate::widget::Widget for Reentrant {
            fn base(&self) -> &crate::widget::BaseWidget {
                &self.base
            }
            fn base_mut(&mut self) -> &mut crate::widget::BaseWidget {
                &mut self.base
            }
        }
        impl EventHandler for Reentrant {
            fn handle_event(&mut self, event: &Event) {
                if matches!(event, Event::ImeCommit { .. }) {
                    // The lock must already be free when the commit is delivered, or the
                    // re-entrant focus_out below would block forever on this same thread.
                    assert!(
                        crate::compat::try_lock(&self.bridge.focused_widget).is_some(),
                        "commit delivery must not hold the focused_widget lock"
                    );
                    self.bridge.focus_out(self.id.get());
                    self.reentered.set(true);
                }
            }
        }

        let id = crate::widget::runtime::register(Box::new(Reentrant {
            bridge: Rc::clone(&bridge),
            id: Rc::clone(&id_cell),
            reentered: Rc::clone(&reentered),
            base: crate::widget::BaseWidget::new(
                crate::widget::WidgetKind::LineEdit,
                crate::core::Rect::new(0, 0, 10, 10),
                "reentrant",
            ),
        }))
        .expect("mount the reentrant widget");
        id_cell.set(id);

        bridge.focus_in(id);
        bridge.commit_text("hi");

        assert!(reentered.get(), "the commit handler must have run");
        assert!(
            lock(&bridge.focused_widget).is_none(),
            "focus_out must have re-entered and cleared"
        );

        crate::widget::runtime::unregister(id);
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
    use core::mem::{align_of, offset_of, size_of};

    /// Every handle-sized slot must be pointer-sized, or the vtable offsets shift.
    #[test]
    fn vtable_slots_are_pointer_sized() {
        assert_eq!(size_of::<usize>(), size_of::<*const core::ffi::c_void>());
        assert_eq!(align_of::<usize>(), align_of::<*const core::ffi::c_void>());
    }

    /// The `IUnknown` vtable is exactly three function pointers.
    #[test]
    fn iunknown_vtbl_is_three_pointers() {
        assert_eq!(size_of::<IUnknownVtbl>(), 3 * size_of::<usize>());
    }

    /// `ITfThreadMgrVtbl`'s used slots must land at the ABI offsets, checked against the real
    /// struct layout rather than re-derived arithmetic: `Activate` is slot 3 (0-based) and
    /// `Deactivate` is slot 4. A missing or misordered field would shift these and make the
    /// bridge call the wrong function through the vtable. `Deactivate` is the last slot
    /// transcribed, so it must sit at the end of the struct.
    #[test]
    fn thread_mgr_slots_match_the_abi() {
        let p = size_of::<usize>();
        assert_eq!(offset_of!(ITfThreadMgrVtbl, activate), 3 * p);
        assert_eq!(offset_of!(ITfThreadMgrVtbl, deactivate), 4 * p);
        assert_eq!(offset_of!(ITfThreadMgrVtbl, deactivate), size_of::<ITfThreadMgrVtbl>() - p);
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
        assert_eq!(IID_ITF_THREAD_MGR.data2, 0x2021);
    }
}
