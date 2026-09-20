// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WASM platform types and runtime state.

use crate::compat::{String, ToString};
use crate::platform::state::BackendState;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;

/// Handle kinds for WASM widgets.
///
/// # Why this has one variant
///
/// It used to enumerate every `WidgetKind` the DOM backend could build an element
/// for, because the host owned a control per kind. The library paints every
/// `WidgetKind` now, so the only thing this backend still allocates a handle for is
/// the **window** it paints into (BLUE15 #55/#56).
///
/// Unlike the mobile backends, this one has no menu variants: the browser gives the
/// host a real menu surface only through DOM elements the library owns, so there is
/// no host-side menu model to track. The enum is kept rather than inlined into
/// `BackendState<u64>` because the state type is generic over it, and collapsing it
/// would make the wasm backend's state shape differ from every other backend for no
/// gain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WasmHandleKind {
    /// Top-level browser window / document.
    Window,
}

/// WASM platform runtime lifecycle state.
pub(crate) struct WasmRuntime {
    pub(crate) initialized: AtomicBool,
    pub(crate) running: AtomicBool,
}

impl WasmRuntime {
    pub(crate) fn new() -> Self {
        Self { initialized: AtomicBool::new(false), running: AtomicBool::new(false) }
    }
}

/// WASM platform backend.
///
/// Stores widget state in a thread-safe `BackendState<WasmHandleKind>` and
/// conditionally interacts with the browser DOM via `web-sys` when compiled
/// for `target_arch = "wasm32"`.
pub struct WasmPlatform {
    pub(crate) state: BackendState<WasmHandleKind>,
    pub(crate) runtime: WasmRuntime,
    pub(crate) canvas_id: String,
    /// The `ResizeObserver` attached by [`WasmPlatform::observe_canvas_resize`], if any.
    ///
    /// Kept so the observation can be replaced or dropped rather than accumulating one
    /// observer per call, which would re-run the window's layout once per attached
    /// observer every time the canvas changes size.
    ///
    /// `Mutex` rather than `RefCell` because `Platform` is `Send + Sync`; the browser
    /// runs this on one thread anyway, so the lock is never contended.
    #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
    pub(crate) resize_observer: crate::compat::Mutex<Option<web_sys::ResizeObserver>>,
}

impl WasmPlatform {
    /// Create a new WASM platform backend with a default canvas id.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            runtime: WasmRuntime::new(),
            canvas_id: "wgpu-canvas".to_string(),
            #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
            resize_observer: crate::compat::Mutex::new(None),
        }
    }

    /// Create a new WASM platform backend with a specific canvas element id.
    pub fn with_canvas(canvas_id: &str) -> Self {
        Self {
            state: BackendState::new(),
            runtime: WasmRuntime::new(),
            canvas_id: canvas_id.to_string(),
            #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
            resize_observer: crate::compat::Mutex::new(None),
        }
    }

    /// Get the HTML canvas element id used for rendering.
    pub fn canvas_id(&self) -> &str {
        &self.canvas_id
    }

    /// Insert a widget record into the state backend and return its allocated id.
    pub(crate) fn insert_widget(
        &self,
        kind: WasmHandleKind,
        text: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> u64 {
        self.state.create_widget(kind, text, x, y, w, h)
    }

    /// Makes `window_id`'s layout follow this backend's canvas as the page resizes it.
    ///
    /// # Why an observer rather than a `window.onresize` listener
    ///
    /// The library's client area is the **canvas**, not the browser window: a page can
    /// resize its canvas without the window changing (a sidebar opens, the canvas is in
    /// a flex column), and the window can change without the canvas being repainted at a
    /// new size. `ResizeObserver` reports the element that actually shrank, which is the
    /// element the library paints into.
    ///
    /// Calling this again replaces the previous observation, so a host that re-runs its
    /// setup does not end up laying the window out once per attached observer.
    ///
    /// Returns `false` when there is no canvas or the browser has no
    /// `ResizeObserver` (an older engine), in which case the host must report resizes
    /// through [`crate::queue_resize_trigger`] itself. `false` is also the answer on a
    /// non-wasm host, where there is no DOM to observe.
    #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
    pub fn observe_canvas_resize(&self, window_id: crate::core::ObjectId) -> bool {
        use wasm_bindgen::JsCast;

        let Some(canvas) = self.canvas_element() else {
            log::warn!(
                "[wasm] observe_canvas_resize: no canvas element with id '{}'; nothing to \
                 observe",
                self.canvas_id
            );
            return false;
        };

        // The callback is handed to JS with `into_js_value`, which transfers ownership to
        // the JS heap. That matters for two reasons: the DOM requires the callback to
        // outlive the `ResizeObserver::new` call, and a Rust-side `Closure` handle is
        // `!Send`, which `Platform` cannot hold. The JS side keeps it alive for as long as
        // the observer is attached; `web_sys`'s own implementation does the same.
        let callback = wasm_bindgen::closure::Closure::<dyn FnMut(js_sys::Array)>::new(
            move |entries: js_sys::Array| {
                for entry in entries.iter() {
                    let Ok(entry) = entry.dyn_into::<web_sys::ResizeObserverEntry>() else {
                        continue;
                    };
                    // The content box is the client area the library lays out against.
                    let rect = entry.content_rect();
                    let width = rect.width().round().max(0.0) as u32;
                    let height = rect.height().round().max(0.0) as u32;
                    if width == 0 || height == 0 {
                        // A hidden or `display: none` canvas reports a zero client area.
                        // Reporting it would lay every child out at zero size; the next
                        // non-zero report re-runs the layout, so skipping is the safe
                        // answer.
                        continue;
                    }
                    crate::queue_resize_trigger(window_id, width, height);
                }
            },
        );

        let callback: js_sys::Function = callback.into_js_value().unchecked_into();
        let Ok(observer) = web_sys::ResizeObserver::new(&callback) else {
            log::warn!(
                "[wasm] observe_canvas_resize: this browser has no ResizeObserver; the host \
                 must report resizes through `queue_resize_trigger` instead"
            );
            return false;
        };
        observer.observe(&canvas);
        // `compat::Mutex` is `std::sync::Mutex` under a std profile and `spin::Mutex`
        // under `mini`. They differ exactly here: `std`'s `lock()` returns a `Result`
        // a caller must recover from on poisoning, while `spin`'s returns the guard
        // directly. Recovering via `into_inner()` is the right behaviour for a
        // poisoned observer slot — the slot holds a JS handle with no invariants to
        // protect — so the std arm keeps it and the spin arm needs no recovery.
        #[cfg(not(alloc_frugal))]
        {
            *self.resize_observer.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some(observer);
        }
        #[cfg(alloc_frugal)]
        {
            *self.resize_observer.lock() = Some(observer);
        }
        true
    }

    /// Makes `window_id`'s layout follow the canvas as the page resizes it.
    ///
    /// Always `false` on a non-wasm host: there is no DOM canvas to observe, so the
    /// honest answer is that the library cannot watch for resizes here — a host that
    /// knows its own size reports it through [`crate::queue_resize_trigger`].
    #[cfg(not(all(target_arch = "wasm32", not(target_os = "wasi"))))]
    pub fn observe_canvas_resize(&self, _window_id: crate::core::ObjectId) -> bool {
        false
    }

    /// Returns this backend's canvas element, if the document has one.
    #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
    fn canvas_element(&self) -> Option<web_sys::HtmlCanvasElement> {
        use wasm_bindgen::JsCast;
        web_sys::window()?
            .document()?
            .get_element_by_id(&self.canvas_id)?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .ok()
    }
}

crate::impl_default_via_new!(WasmPlatform);
