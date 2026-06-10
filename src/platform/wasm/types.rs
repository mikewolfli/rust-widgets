//! WASM platform types and runtime state.

use crate::platform::state::BackendState;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

/// Handle kinds for WASM widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WasmHandleKind {
    Window,
    Button,
    CheckBox,
    LineEdit,
    Label,
    RadioButton,
    Slider,
    ProgressBar,
    ComboBox,
    ListBox,
    Panel,
    MenuBar,
    Menu,
    MenuItem,
    ToolBar,
    StatusBar,
    MessageBox,
    FileDialog,
    ColorDialog,
    FontDialog,
    SpinBox,
    ListView,
    ScrollArea,
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

    /// Return the handle kind of an existing widget, if any.
    pub(crate) fn kind_of(&self, id: u64) -> Option<WasmHandleKind> {
        self.state.kind_of(id)
    }
}

impl Default for WasmPlatform {
    fn default() -> Self {
        Self::new()
    }
}
