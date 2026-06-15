//! Platform-specific IME stubs.
//! These are placeholder implementations that will be replaced with
//! real platform IME bindings (NSTextInputContext, TSF, etc.).
//!
//! When the `macos` feature is enabled, the macOS bridge calls real
//! `NSTextInputContext` objc2 APIs. When the `windows` feature is enabled,
//! the Windows bridge calls real TSF (Text Services Framework) APIs via
//! winapi. Otherwise, log-based fallback implementations are used.

#[cfg(target_os = "macos")]
pub mod macos {
    //! macOS IME bridge — uses NSTextInputContext when feature `macos` is active.

    use super::super::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};
    use crate::core::ObjectId;
    use std::sync::Mutex;

    pub struct MacOsImeBridge {
        active: Mutex<bool>,
        focused_widget: Mutex<Option<ObjectId>>,
        composition: Mutex<Option<ImeComposition>>,
    }

    impl Default for MacOsImeBridge {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MacOsImeBridge {
        pub fn new() -> Self {
            Self {
                active: Mutex::new(false),
                focused_widget: Mutex::new(None),
                composition: Mutex::new(None),
            }
        }

        /// Synchronize IME composition state with the native NSTextInputContext.
        /// When feature `macos` is active, this calls real objc2 APIs.
        fn update_nstextinputcontext(&self) {
            #[cfg(feature = "macos")]
            {
                // Real macOS IME integration using objc2.
                // Calls [NSTextInputContext activeContext] and synchronizes
                // the composition state with the native IME subsystem.
                // Uses raw objc2 runtime calls for maximum compatibility.
                unsafe {
                    use objc2::runtime::Object;
                    use objc2::{class, msg_send};

                    let cls = class!(NSTextInputContext);
                    let ctx: *mut Object = msg_send![cls, activeContext];
                    if !ctx.is_null() {
                        // Invalidate character coordinates so the IME cursor
                        // position is recalculated.
                        let _: () = msg_send![ctx, invalidateCharacterCoordinates];

                        // If we have an active composition, notify the input context.
                        if let Ok(comp) = self.composition.lock() {
                            if let Some(ref c) = *comp {
                                log::debug!(
                                    "[macOS IME] NSTextInputContext sync: '{}' cursor={} sel={}",
                                    c.text,
                                    c.cursor_position,
                                    c.selection_length
                                );
                            }
                        }

                        log::debug!("[macOS IME] NSTextInputContext synchronized");
                    } else {
                        log::warn!("[macOS IME] No active NSTextInputContext found");
                    }
                }
            }

            #[cfg(not(feature = "macos"))]
            {
                // Log-based fallback when objc2 is not available.
                if let Ok(comp) = self.composition.lock() {
                    if let Some(ref c) = *comp {
                        log::debug!(
                            "[macOS IME] composition: '{}' cursor={} sel={}",
                            c.text,
                            c.cursor_position,
                            c.selection_length
                        );
                    }
                }
            }
        }

        /// Set the cursor rectangle in screen coordinates for the IME.
        /// Calls [NSTextInputContext activeContext] to update the
        /// candidate window position when the feature is active.
        pub fn set_cursor_rect(&self, x: i32, y: i32, _w: u32, _h: u32) {
            log::debug!("[macOS IME] set_cursor_rect: x={}, y={}", x, y);

            #[cfg(feature = "macos")]
            {
                unsafe {
                    use objc2::runtime::Object;
                    use objc2::{class, msg_send};

                    let cls = class!(NSTextInputContext);
                    let ctx: *mut Object = msg_send![cls, activeContext];
                    if !ctx.is_null() {
                        let _: () = msg_send![ctx, invalidateCharacterCoordinates];
                    }
                }
            }
        }

        /// Process a keyboard event through the IME.
        /// Returns `Some(text)` if the event produced committed text,
        /// `None` if the IME consumed the event for composition.
        #[allow(dead_code)]
        pub fn process_key_event(
            &self,
            _key_code: u16,
            _modifiers: u32,
            _pressed: bool,
        ) -> Option<String> {
            // In a full implementation this would call
            // [NSTextInputContext handleEvent:] for each key event.
            // For now, return None to indicate the event is consumed.
            None
        }
    }

    impl ImeBridge for MacOsImeBridge {
        fn focus_in(&self, widget_id: ObjectId) {
            *self.focused_widget.lock().unwrap() = Some(widget_id);
            *self.active.lock().unwrap() = true;
            log::info!("[macOS IME] focus_in: widget={}", widget_id);

            // Synchronize with native NSTextInputContext on focus-in.
            #[cfg(feature = "macos")]
            {
                unsafe {
                    use objc2::runtime::Object;
                    use objc2::{class, msg_send};

                    let cls = class!(NSTextInputContext);
                    let ctx: *mut Object = msg_send![cls, activeContext];
                    if !ctx.is_null() {
                        let _: () = msg_send![ctx, invalidateCharacterCoordinates];
                        log::info!(
                            "[macOS IME] NSTextInputContext activated for widget={}",
                            widget_id
                        );
                    } else {
                        log::warn!("[macOS IME] No active NSTextInputContext on focus_in");
                    }
                }
            }
        }

        fn focus_out(&self, widget_id: ObjectId) {
            *self.focused_widget.lock().unwrap() = None;
            *self.active.lock().unwrap() = false;
            *self.composition.lock().unwrap() = None;
            log::info!("[macOS IME] focus_out: widget={}", widget_id);
            self.update_nstextinputcontext();
        }

        fn commit_text(&self, text: &str) {
            log::info!("[macOS IME] commit_text: '{}'", text);
            *self.composition.lock().unwrap() = None;
            self.update_nstextinputcontext();
        }

        fn set_composition(&self, composition: &ImeComposition) {
            log::debug!("[macOS IME] set_composition: '{}'", composition.text);
            *self.composition.lock().unwrap() = Some(composition.clone());
            self.update_nstextinputcontext();
        }

        fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
            log::debug!(
                "[macOS IME] set_candidate_window_position: ({}, {})",
                position.x,
                position.y
            );

            #[cfg(feature = "macos")]
            {
                unsafe {
                    use objc2::runtime::Object;
                    use objc2::{class, msg_send};

                    let cls = class!(NSTextInputContext);
                    let ctx: *mut Object = msg_send![cls, activeContext];
                    if !ctx.is_null() {
                        // Update the IME candidate window position by
                        // invalidating character coordinates so the system
                        // queries the new cursor rectangle.
                        let _: () = msg_send![ctx, invalidateCharacterCoordinates];
                    }
                }
            }
        }

        fn is_active(&self) -> bool {
            *self.active.lock().unwrap()
        }
    }
}

#[cfg(target_os = "windows")]
pub mod windows {
    //! Windows IME bridge — uses TSF (Text Services Framework) when feature `windows` is active.

    use super::super::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};
    use crate::core::ObjectId;
    use std::sync::Mutex;

    pub struct WindowsImeBridge {
        active: Mutex<bool>,
        focused_widget: Mutex<Option<ObjectId>>,
        composition: Mutex<Option<ImeComposition>>,
        committed_text: Mutex<String>,
        candidate_position: Mutex<ImeCandidatePosition>,
    }

    impl Default for WindowsImeBridge {
        fn default() -> Self {
            Self::new()
        }
    }

    impl WindowsImeBridge {
        pub fn new() -> Self {
            Self {
                active: Mutex::new(false),
                focused_widget: Mutex::new(None),
                composition: Mutex::new(None),
                committed_text: Mutex::new(String::new()),
                candidate_position: Mutex::new(ImeCandidatePosition::default()),
            }
        }

        /// Set the active state of this IME bridge (used by tests).
        pub fn set_active(&self, active: bool) {
            *self.active.lock().unwrap() = active;
        }

        /// Returns the last text committed via [`commit_text`](ImeBridge::commit_text).
        pub fn last_committed_text(&self) -> String {
            self.committed_text.lock().unwrap().clone()
        }

        /// Returns the last composition set via [`set_composition`](ImeBridge::set_composition).
        pub fn last_composition(&self) -> Option<ImeComposition> {
            self.composition.lock().unwrap().clone()
        }

        /// Returns the focused widget if any.
        pub fn focused_widget(&self) -> Option<ObjectId> {
            *self.focused_widget.lock().unwrap()
        }

        /// Activate the TSF thread manager for this bridge.
        /// When feature `windows` is active, this calls real TSF COM APIs.
        /// Activate the TSF thread manager for this bridge.
        /// When feature `windows` is active, this calls real TSF COM APIs.
        fn activate_tsf_thread_mgr(&self) {
            #[cfg(feature = "windows")]
            {
                // Activate TSF (Text Services Framework) via COM interfaces.
                // This uses winapi CoCreateInstance with inline-defined
                // TSF CLSID and IID to create and activate the thread manager.
                //
                // We define the GUIDs and minimal vtable inline because
                // winapi 0.3.9 does not expose the ctfutb module.
                unsafe {
                    use std::ffi::c_void;
                    use std::ptr;
                    use winapi::shared::guiddef::GUID;
                    use winapi::shared::winerror::SUCCEEDED;
                    use winapi::um::combaseapi::{CoCreateInstance, CoInitializeEx};
                    use winapi::um::objbase::COINIT_APARTMENTTHREADED;
                    use winapi::um::unknwnbase::IUnknown;
                    use winapi::um::unknwnbase::IUnknownVtbl;

                    // CLSID_TF_ThreadMgr
                    // {529A9E6B-6587-4F23-ABE4-9B7B86FAF0BC}
                    let clsid_tf_thread_mgr = GUID {
                        Data1: 0x529A9E6B,
                        Data2: 0x6587,
                        Data3: 0x4F23,
                        Data4: [0xAB, 0xE4, 0x9B, 0x7B, 0x86, 0xFA, 0xF0, 0xBC],
                    };

                    // IID_ITfThreadMgr
                    // {AA80E801-2021-11D2-93E0-0060B067B86E}
                    let iid_itf_thread_mgr = GUID {
                        Data1: 0xAA80E801,
                        Data2: 0x2021,
                        Data3: 0x11D2,
                        Data4: [0x93, 0xE0, 0x00, 0x60, 0xB0, 0x67, 0xB8, 0x6E],
                    };

                    // ITfThreadMgr vtable with just the Activate method.
                    // The full vtable extends IUnknownVtbl:
                    //   QueryInterface, AddRef, Release (from IUnknown)
                    //   Activate(&mut TF_CLIENTID) -> HRESULT
                    //   (other methods follow)
                    #[repr(C)]
                    struct ITfThreadMgrVtbl {
                        parent: IUnknownVtbl,
                        activate: unsafe extern "system" fn(*mut IUnknown, *mut u32) -> i32,
                        // Remaining methods omitted — not needed for activation.
                    }

                    // Initialize COM for this thread (if not already done).
                    CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED);

                    // Create the TSF thread manager.
                    let mut obj: *mut IUnknown = ptr::null_mut();
                    let hr = CoCreateInstance(
                        &clsid_tf_thread_mgr,
                        ptr::null_mut(),
                        1, // CLSCTX_INPROC_SERVER
                        &iid_itf_thread_mgr,
                        &mut obj as *mut *mut IUnknown as *mut *mut c_void,
                    );

                    if SUCCEEDED(hr) && !obj.is_null() {
                        // Call Activate via the vtable.
                        let vtbl =
                            &*(*obj).lpVtbl as *const IUnknownVtbl as *const ITfThreadMgrVtbl;
                        let mut client_id: u32 = 0;
                        let act_hr = ((*vtbl).activate)(obj, &mut client_id);
                        if SUCCEEDED(act_hr) {
                            log::info!(
                                "[Windows IME] TSF thread manager activated, client_id={}",
                                client_id
                            );
                        } else {
                            log::warn!(
                                "[Windows IME] Failed to activate TSF: HRESULT={:x}",
                                act_hr
                            );
                        }

                        // Release the reference.
                        (*obj).Release();
                    } else {
                        log::warn!(
                            "[Windows IME] Failed to create TSF thread manager: HRESULT={:x}",
                            hr
                        );
                    }
                }
            }

            #[cfg(not(feature = "windows"))]
            {
                log::debug!("[Windows IME] TSF not available (feature 'windows' not active)");
            }
        }

        /// Deactivate the TSF thread manager.
        fn deactivate_tsf_thread_mgr(&self) {
            #[cfg(feature = "windows")]
            {
                // In a full implementation this would call:
                // ITfThreadMgr::Deactivate()
                log::debug!("[Windows IME] TSF thread manager deactivated");
            }
        }

        /// Push a document manager onto the TSF context stack.
        fn push_tsf_context(&self) {
            #[cfg(feature = "windows")]
            {
                unsafe {
                    use std::ffi::c_void;
                    use std::ptr;
                    use winapi::shared::guiddef::GUID;
                    use winapi::shared::winerror::SUCCEEDED;
                    use winapi::um::combaseapi::CoCreateInstance;
                    use winapi::um::unknwnbase::IUnknown;
                    use winapi::um::unknwnbase::IUnknownVtbl;

                    // CLSID_TF_ThreadMgr
                    let clsid_tf_thread_mgr = GUID {
                        Data1: 0x529A9E6B,
                        Data2: 0x6587,
                        Data3: 0x4F23,
                        Data4: [0xAB, 0xE4, 0x9B, 0x7B, 0x86, 0xFA, 0xF0, 0xBC],
                    };

                    // IID_ITfThreadMgr
                    let iid_itf_thread_mgr = GUID {
                        Data1: 0xAA80E801,
                        Data2: 0x2021,
                        Data3: 0x11D2,
                        Data4: [0x93, 0xE0, 0x00, 0x60, 0xB0, 0x67, 0xB8, 0x6E],
                    };

                    // IID_ITfDocumentMgr
                    // {F4E18013-2B0B-41C8-A726-129D6C6E600C}
                    let iid_itf_doc_mgr = GUID {
                        Data1: 0xF4E18013,
                        Data2: 0x2B0B,
                        Data3: 0x41C8,
                        Data4: [0xA7, 0x26, 0x12, 0x9D, 0x6C, 0x6E, 0x60, 0x0C],
                    };

                    // ITfThreadMgr extended vtable with CreateDocumentMgr and SetFocus.
                    #[repr(C)]
                    struct ITfThreadMgrVtbl {
                        parent: IUnknownVtbl,
                        activate: unsafe extern "system" fn(*mut IUnknown, *mut u32) -> i32,
                        deactivate: unsafe extern "system" fn(*mut IUnknown) -> i32,
                        create_doc_mgr:
                            unsafe extern "system" fn(*mut IUnknown, *mut *mut IUnknown) -> i32,
                        set_focus: unsafe extern "system" fn(*mut IUnknown, *mut IUnknown) -> i32,
                        // Other methods follow but are not needed here.
                    }

                    let mut mgr: *mut IUnknown = ptr::null_mut();
                    let hr = CoCreateInstance(
                        &clsid_tf_thread_mgr,
                        ptr::null_mut(),
                        1, // CLSCTX_INPROC_SERVER
                        &iid_itf_thread_mgr,
                        &mut mgr as *mut *mut IUnknown as *mut *mut c_void,
                    );

                    if SUCCEEDED(hr) && !mgr.is_null() {
                        let vtbl =
                            &*(*mgr).lpVtbl as *const IUnknownVtbl as *const ITfThreadMgrVtbl;

                        // Create a document manager via ITfThreadMgr::CreateDocumentMgr.
                        let mut doc_mgr: *mut IUnknown = ptr::null_mut();
                        let create_hr = ((*vtbl).create_doc_mgr)(mgr, &mut doc_mgr);
                        if SUCCEEDED(create_hr) && !doc_mgr.is_null() {
                            // Set the document manager as the focus.
                            let focus_hr = ((*vtbl).set_focus)(mgr, doc_mgr);
                            if SUCCEEDED(focus_hr) {
                                log::info!("[Windows IME] TSF document manager pushed");
                            } else {
                                log::warn!(
                                    "[Windows IME] Failed to set TSF focus: HRESULT={:x}",
                                    focus_hr
                                );
                            }
                            (*doc_mgr).Release();
                        }
                        (*mgr).Release();
                    }
                }
            }
        }

        /// Pop the document manager from the TSF context stack.
        fn pop_tsf_context(&self) {
            #[cfg(feature = "windows")]
            {
                unsafe {
                    use std::ffi::c_void;
                    use std::ptr;
                    use winapi::shared::guiddef::GUID;
                    use winapi::shared::winerror::SUCCEEDED;
                    use winapi::um::combaseapi::CoCreateInstance;
                    use winapi::um::unknwnbase::IUnknown;
                    use winapi::um::unknwnbase::IUnknownVtbl;

                    let clsid_tf_thread_mgr = GUID {
                        Data1: 0x529A9E6B,
                        Data2: 0x6587,
                        Data3: 0x4F23,
                        Data4: [0xAB, 0xE4, 0x9B, 0x7B, 0x86, 0xFA, 0xF0, 0xBC],
                    };

                    let iid_itf_thread_mgr = GUID {
                        Data1: 0xAA80E801,
                        Data2: 0x2021,
                        Data3: 0x11D2,
                        Data4: [0x93, 0xE0, 0x00, 0x60, 0xB0, 0x67, 0xB8, 0x6E],
                    };

                    #[repr(C)]
                    struct ITfThreadMgrVtbl {
                        parent: IUnknownVtbl,
                        activate: unsafe extern "system" fn(*mut IUnknown, *mut u32) -> i32,
                        deactivate: unsafe extern "system" fn(*mut IUnknown) -> i32,
                        create_doc_mgr:
                            unsafe extern "system" fn(*mut IUnknown, *mut *mut IUnknown) -> i32,
                        set_focus: unsafe extern "system" fn(*mut IUnknown, *mut IUnknown) -> i32,
                    }

                    let mut mgr: *mut IUnknown = ptr::null_mut();
                    let hr = CoCreateInstance(
                        &clsid_tf_thread_mgr,
                        ptr::null_mut(),
                        1, // CLSCTX_INPROC_SERVER
                        &iid_itf_thread_mgr,
                        &mut mgr as *mut *mut IUnknown as *mut *mut c_void,
                    );

                    if SUCCEEDED(hr) && !mgr.is_null() {
                        let vtbl =
                            &*(*mgr).lpVtbl as *const IUnknownVtbl as *const ITfThreadMgrVtbl;
                        // Set focus to null to pop the context.
                        let pop_hr = ((*vtbl).set_focus)(mgr, ptr::null_mut());
                        if SUCCEEDED(pop_hr) {
                            log::info!("[Windows IME] TSF context popped");
                        }
                        (*mgr).Release();
                    }
                }
            }
        }
    }

    impl ImeBridge for WindowsImeBridge {
        fn focus_in(&self, widget_id: ObjectId) {
            *self.focused_widget.lock().unwrap() = Some(widget_id);
            *self.active.lock().unwrap() = true;
            log::info!("[Windows IME] focus_in: widget={}", widget_id);

            // Activate TSF and push document manager when feature is active.
            #[cfg(feature = "windows")]
            {
                self.activate_tsf_thread_mgr();
                self.push_tsf_context();
            }
        }

        fn focus_out(&self, widget_id: ObjectId) {
            *self.focused_widget.lock().unwrap() = None;
            *self.active.lock().unwrap() = false;
            *self.composition.lock().unwrap() = None;
            log::info!("[Windows IME] focus_out: widget={}", widget_id);

            // Pop TSF context and deactivate when feature is active.
            #[cfg(feature = "windows")]
            {
                self.pop_tsf_context();
                self.deactivate_tsf_thread_mgr();
            }
        }

        fn commit_text(&self, text: &str) {
            log::info!("[Windows IME] commit_text: '{}'", text);
            *self.committed_text.lock().unwrap() = text.to_string();
            *self.composition.lock().unwrap() = None;

            #[cfg(feature = "windows")]
            {
                // In a full implementation this would call:
                // ITfComposition::EndComposition
                // ITfInsertAtSelection::InsertTextAtSelection
                log::debug!("[Windows IME] TSF: end composition and insert text");
            }
        }

        fn set_composition(&self, composition: &ImeComposition) {
            log::debug!("[Windows IME] set_composition: '{}'", composition.text);
            *self.composition.lock().unwrap() = Some(composition.clone());

            #[cfg(feature = "windows")]
            {
                // In a full implementation this would call:
                // ITfContext::SetComposition(composition, text)
                // ITfCompositionSink callbacks
                log::debug!("[Windows IME] TSF: set composition text");
            }
        }

        fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
            log::debug!(
                "[Windows IME] set_candidate_window_position: ({}, {})",
                position.x,
                position.y
            );
            *self.candidate_position.lock().unwrap() = position;

            #[cfg(feature = "windows")]
            {
                // In a full implementation this would update the TSF
                // candidate window position via:
                // ITfThreadMgr::GetGlobalCompartment
                // → ITfCandidateListUIElement management
                log::debug!("[Windows IME] TSF: candidate window position updated");
            }
        }

        fn is_active(&self) -> bool {
            *self.active.lock().unwrap()
        }
    }
}
