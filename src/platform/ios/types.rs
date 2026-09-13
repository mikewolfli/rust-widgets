// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! iOS mobile platform types and state container.
//!
//! This module provides state-backed platform implementation for iOS,
//! serving as a foundation for progressive UIKit/SwiftUI integration.

use crate::platform::state::BackendState;
#[cfg(all(feature = "serde", not(any(feature = "mini", feature = "embedded"))))]
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// iOS-specific widget handle type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    all(feature = "serde", not(any(feature = "mini", feature = "embedded"))),
    derive(Serialize, Deserialize)
)]
pub(crate) enum IosHandleKind {
    /// Top-level window.
    Window,
    /// Push button control.
    Button,
    /// Toggleable checkbox control (UI Switch on iOS).
    CheckBox,
    /// Single-line editable text input.
    LineEdit,
    /// Static text label.
    Label,
    /// Exclusive selection radio button.
    RadioButton,
    /// Range slider.
    Slider,
    /// Determinate/indeterminate progress indicator.
    ProgressBar,
    /// Drop-down selection control (UI Picker on iOS).
    ComboBox,
    /// List selection control (UI TableView on iOS).
    ListBox,
    /// Numeric stepper/edit control.
    SpinBox,
    /// List/table view control.
    ListView,
    /// Scrollable content region.
    ScrollArea,
    /// Generic container panel.
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
    /// File open/save dialog (not standard on iOS).
    FileDialog,
    /// Color picker dialog (UI ColorPickerViewController on iOS).
    ColorDialog,
    /// Font selection dialog.
    FontDialog,
    GroupBox,
    Frame,
    TabWidget,
    Splitter,
    ToggleButton,
    Calendar,
    ScrollBar,
    DoubleSpinBox,
    FontComboBox,
    ContextMenu,
    PopupWindow,
    Dialog,
    InputDialog,
    ProgressDialog,
    DirectoryDialog,
    DatePicker,
    TimePicker,
    DateTimePicker,
    ActivityIndicator,
}

/// List storage state for ComboBox and ListBox.
#[derive(Default)]
pub(crate) struct ListData {
    /// Ordered item text entries.
    pub(crate) items: Vec<String>,
    /// Currently selected index, if any.
    pub(crate) current_index: Option<usize>,
}

/// iOS platform menu state.
#[derive(Default)]
pub(crate) struct IosMenuState {
    /// Window id -> attached menu bar id mapping.
    pub(crate) attached_menu_bar: HashMap<u64, u64>,
    /// Parent menu id -> direct child menu/menu-item ids.
    pub(crate) menu_children: HashMap<u64, Vec<u64>>,
    /// FIFO queue for menu item trigger ids.
    pub(crate) pending_menu_events: VecDeque<u64>,
}

/// iOS platform runtime state lifecycle markers.
pub(crate) struct IosRuntimeState {
    /// `true` after backend initialization has completed.
    pub(crate) initialized: std::sync::atomic::AtomicBool,
    /// `true` while the preview loop is running.
    pub(crate) running: std::sync::atomic::AtomicBool,
}

impl IosRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            initialized: std::sync::atomic::AtomicBool::new(false),
            running: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

/// State-backed iOS platform adapter.
///
/// This backend provides a deterministic, state-driven implementation
/// behind the `mobile-api` feature flag, enabling progressive integration
/// with native UIKit/SwiftUI without requiring full native bindings upfront.
///
/// All widget state is stored in `BackendState<IosHandleKind>`, and
/// platform contract methods translate between Rust API and state mutations.
pub struct IosMobilePlatform {
    /// Internal state for all widgets and handles.
    pub(crate) state: BackendState<IosHandleKind>,
    /// Menu state for menu bar/menu/menu items.
    pub(crate) menus: Mutex<IosMenuState>,
    /// Runtime state for init/run/quit.
    pub(crate) runtime: IosRuntimeState,
    /// Shared list storage for ListBox widgets.
    pub(crate) list_data: Mutex<HashMap<u64, ListData>>,
    /// Shared list storage for ComboBox widgets.
    pub(crate) combo_data: Mutex<HashMap<u64, ListData>>,
    /// Native root view handle attached via `MobilePlatformExtension`.
    pub(crate) attached_native_view: std::sync::atomic::AtomicUsize,
}

impl IosMobilePlatform {
    /// Creates a new iOS state-backed platform backend.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            menus: Mutex::new(IosMenuState::default()),
            runtime: IosRuntimeState::new(),
            list_data: Mutex::new(HashMap::new()),
            combo_data: Mutex::new(HashMap::new()),
            attached_native_view: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Returns the currently attached native root view handle, if any.
    pub fn attached_native_view(&self) -> Option<usize> {
        let handle = self.attached_native_view.load(std::sync::atomic::Ordering::SeqCst);
        if handle == 0 {
            None
        } else {
            Some(handle)
        }
    }

    /// Serialize all widget state for parity/regression testing.
    ///
    /// Mirrors the `BackendState` serde gate exactly: the state type only derives
    /// `Serialize` under `serde` and outside the alloc-free `mini`/`embedded`
    /// profiles, so the method must not exist where the bound cannot hold.
    #[cfg(all(
        feature = "serde_json",
        feature = "serde",
        not(any(feature = "mini", feature = "embedded"))
    ))]
    pub fn serialize_state(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.state)
    }

    pub(crate) fn insert_widget(
        &self,
        kind: IosHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.state.create_widget(kind, text, x, y, width, height)
    }

    pub(crate) fn kind_of(&self, id: u64) -> Option<IosHandleKind> {
        self.state.kind_of(id)
    }

    pub(crate) fn ios_runtime_marker(&self) -> usize {
        // Marker for iOS platform backend
        0
    }

    /// Returns `true` when `ios-uikit-ffi` feature is enabled
    /// and real UIKit views are being created.
    #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
    pub fn ui_kit_available(&self) -> bool {
        true
    }

    /// Returns `false` — no real UIKit FFI bindings are wired yet.
    ///
    /// Once UIKit FFI is integrated (e.g. via `objc2` crates), this
    /// should return `true` and the creation methods should additionally
    /// construct and return native `UIView` handles.
    ///
    /// # Integration Path
    ///
    /// 1. Add `objc2` and `objc2-foundation` / `objc2-ui-kit` dependencies.
    /// 2. Replace each `insert_widget` call with real `UIView` creation:
    ///    - `UIButton` for `Button`
    ///    - `UILabel` for `Label`
    ///    - `UIWindow` for `Window`
    ///    - etc.
    /// 3. Use `objc_id::Id<Object>` or `*mut Object` as the handle value.
    /// 4. Gate the real FFI code behind `#[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]`.
    #[cfg(not(feature = "ios-uikit-ffi"))]
    pub fn ui_kit_available(&self) -> bool {
        false
    }
}

impl Default for IosMobilePlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::platform::types::MobilePlatformExtension for IosMobilePlatform {
    fn mobile_backend(&self) -> crate::platform::types::MobileBackend {
        crate::platform::types::MobileBackend::Ios
    }

    fn attach_to_native_view(&self, native_handle: usize) -> bool {
        // The handle is the host `UIWindow`/root `UIView` provided by the app
        // delegate. It is recorded so the runtime can report the attached view;
        // UIKit objects themselves are retained by the Objective-C runtime.
        if native_handle == 0 {
            return false;
        }
        self.attached_native_view.store(native_handle, std::sync::atomic::Ordering::SeqCst);
        true
    }
}
