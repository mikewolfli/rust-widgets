// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WASM platform types and runtime state.

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
}

impl WasmPlatform {
    /// Create a new WASM platform backend with a default canvas id.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            runtime: WasmRuntime::new(),
            canvas_id: "wgpu-canvas".to_string(),
        }
    }

    /// Create a new WASM platform backend with a specific canvas element id.
    pub fn with_canvas(canvas_id: &str) -> Self {
        Self {
            state: BackendState::new(),
            runtime: WasmRuntime::new(),
            canvas_id: canvas_id.to_string(),
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
}

impl Default for WasmPlatform {
    fn default() -> Self {
        Self::new()
    }
}
