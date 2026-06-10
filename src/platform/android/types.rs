//! Android platform types and state container.
//!
//! This module provides a state-backed platform implementation for Android,
//! serving as a foundation for progressive JNI native view integration.

use crate::platform::state::BackendState;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

/// Android-specific widget handle type discriminator.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum AndroidHandleKind {
    /// Top-level window (Android Activity / Dialog).
    Window,
    /// Push button control.
    Button,
    /// Toggleable checkbox control.
    CheckBox,
    /// Single-line editable text input.
    LineEdit,
    /// Static text label.
    Label,
    /// Exclusive selection radio button.
    RadioButton,
    /// Range slider (SeekBar on Android).
    Slider,
    /// Determinate/indeterminate progress indicator.
    ProgressBar,
    /// Drop-down selection control (Spinner on Android).
    ComboBox,
    /// List selection control (ListView on Android).
    ListBox,
    /// Generic container panel (ViewGroup on Android).
    Panel,
    /// Root menu bar container.
    MenuBar,
    /// Hierarchical menu node.
    Menu,
    /// Actionable menu leaf item.
    MenuItem,
    /// Window toolbar region.
    ToolBar,
    /// Window status bar region.
    StatusBar,
    /// Modal message box dialog.
    MessageBox,
    /// File open/save dialog.
    FileDialog,
    /// Color picker dialog.
    ColorDialog,
    /// Font selection dialog.
    FontDialog,
    /// Spin box (numeric value picker).
    SpinBox,
    /// List view (multi-column / detailed list).
    ListView,
    /// Scrollable content area.
    ScrollArea,
}

/// List storage state for ComboBox and ListBox.
#[derive(Default)]
pub(crate) struct AndroidListData {
    /// Ordered item text entries.
    pub(crate) items: Vec<String>,
    /// Currently selected index, if any.
    pub(crate) current_index: Option<usize>,
}

/// Android platform menu state.
#[derive(Default)]
pub(crate) struct AndroidMenuState {
    /// Window id -> attached menu bar id mapping.
    pub(crate) attached_menu_bar: HashMap<u64, u64>,
    /// Parent menu id -> direct child menu/menu-item ids.
    pub(crate) menu_children: HashMap<u64, Vec<u64>>,
    /// FIFO queue for menu item trigger ids.
    pub(crate) pending_menu_events: VecDeque<u64>,
}

/// Android platform runtime state lifecycle markers.
pub(crate) struct AndroidRuntimeState {
    /// `true` after backend initialization has completed.
    pub(crate) initialized: AtomicBool,
    /// `true` while the run loop is active.
    pub(crate) running: AtomicBool,
}

impl AndroidRuntimeState {
    pub(crate) fn new() -> Self {
        Self { initialized: AtomicBool::new(false), running: AtomicBool::new(false) }
    }
}

impl Default for AndroidRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

/// State-backed Android platform adapter.
///
/// This backend provides a deterministic, state-driven implementation
/// behind the `target_os = "android"` cfg gate, enabling progressive
/// integration with native Android views via JNI without requiring
/// full native bindings upfront.
///
/// All widget state is stored in `BackendState<AndroidHandleKind>`, and
/// platform contract methods translate between Rust API and state mutations.
/// When the `android-jni` feature is enabled, widget creation and mutation
/// methods additionally call into the JNI bridge to create/manage real
/// Android native `View` objects.
pub struct AndroidPlatform {
    /// Internal state for all widgets and handles.
    pub(crate) state: BackendState<AndroidHandleKind>,
    /// Menu state for menu bar/menu/menu items.
    pub(crate) menus: Mutex<AndroidMenuState>,
    /// Runtime state for init/run/quit.
    pub(crate) runtime: AndroidRuntimeState,
    /// Shared list storage for ComboBox and ListBox widgets.
    pub(crate) list_data: Mutex<HashMap<u64, AndroidListData>>,
    /// Optional JVM pointer (set via `init_jvm`).
    pub(crate) jvm: Option<*mut std::ffi::c_void>,
}

// Safety: `jvm` is a raw pointer only used within JNI calls that are
// inherently unsafe and require external synchronization.
unsafe impl Send for AndroidPlatform {}
unsafe impl Sync for AndroidPlatform {}

impl AndroidPlatform {
    /// Creates a new Android state-backed platform backend.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            menus: Mutex::new(AndroidMenuState::default()),
            runtime: AndroidRuntimeState::new(),
            list_data: Mutex::new(HashMap::new()),
            jvm: None,
        }
    }

    /// Initialize JVM pointer for JNI-based native view operations.
    pub fn init_jvm(&mut self, jvm: *mut std::ffi::c_void) {
        self.jvm = Some(jvm);
    }

    /// Check if JNI is available (JVM pointer set + JNI feature enabled).
    pub fn jni_available(&self) -> bool {
        #[cfg(feature = "android-jni")]
        {
            self.jvm.is_some() && crate::platform::android_jni::is_initialized()
        }
        #[cfg(not(feature = "android-jni"))]
        {
            let _ = self.jvm;
            false
        }
    }

    /// Serialize all widget state for parity/regression testing.
    pub fn serialize_state(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.state)
    }

    pub(crate) fn insert_widget(
        &self,
        kind: AndroidHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.state.create_widget(kind, text, x, y, width, height)
    }

    pub(crate) fn kind_of(&self, id: u64) -> Option<AndroidHandleKind> {
        self.state.kind_of(id)
    }

    pub(crate) fn android_runtime_marker(&self) -> usize {
        // Marker for Android platform backend
        0
    }
}

impl Default for AndroidPlatform {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_android_platform_new() {
        let platform = AndroidPlatform::new();
        assert!(platform.jvm.is_none());
        assert!(!platform.runtime.initialized.load(Ordering::SeqCst));
        assert!(!platform.runtime.running.load(Ordering::SeqCst));
    }

    #[test]
    fn test_android_platform_jni_not_available_by_default() {
        let platform = AndroidPlatform::new();
        // Without JVM set, JNI should not be available
        assert!(!platform.jni_available());
    }

    #[test]
    fn test_android_platform_insert_widget() {
        let platform = AndroidPlatform::new();
        let id = platform.insert_widget(AndroidHandleKind::Button, "Click", 10, 20, 100, 30);
        assert_ne!(id, 0);
        assert_eq!(platform.kind_of(id), Some(AndroidHandleKind::Button));
    }
}
