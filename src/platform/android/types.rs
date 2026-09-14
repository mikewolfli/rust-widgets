// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Android platform types and state container.
//!
//! This module provides a state-backed platform implementation for Android,
//! serving as a foundation for progressive JNI native view integration.

use crate::platform::state::BackendState;
#[cfg(all(feature = "serde", widgets_unstripped))]
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

/// Android-specific widget handle type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub(crate) enum AndroidHandleKind {
    /// Top-level window (Android Activity / Dialog).
    Window,
    /// Root menu bar bound to the host Activity's own menu.
    MenuBar,
    /// Hierarchical menu node.
    Menu,
    /// Actionable menu leaf item.
    MenuItem,
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
/// behind the `target_os = "android"` cfg gate.
///
/// All widget state is stored in `BackendState<AndroidHandleKind>`, and
/// platform contract methods translate between Rust API and state mutations.
///
/// # BLUE15: the host no longer builds Android views
///
/// Widget creation used to call into JNI to instantiate a real `android.view.View`
/// per logical widget. Under the self-drawn strategy the host owes the widget layer
/// a window and a drawing surface, and the library paints every `WidgetKind`, so a
/// per-kind `create_*` here has no OS object to map onto (BLUE15 #56). The state
/// model is retained because it is what the host's window and event plumbing is
/// expressed in.
pub struct AndroidPlatform {
    /// Internal state for all widgets and handles.
    pub(crate) state: BackendState<AndroidHandleKind>,
    /// Menu state for menu bar/menu/menu items.
    pub(crate) menus: Mutex<AndroidMenuState>,
    /// Runtime state for init/run/quit.
    pub(crate) runtime: AndroidRuntimeState,
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
            jvm: None,
        }
    }

    /// Initialize JVM pointer for JNI-based operations.
    pub fn init_jvm(&mut self, jvm: *mut std::ffi::c_void) {
        self.jvm = Some(jvm);
    }

    /// Check whether the JNI bridge can be reached.
    ///
    /// True when the `android-jni` feature is enabled, the JNI bridge has a
    /// `JavaVM` (`nativeInit` ran), and an Activity `Context` has been stored
    /// (via `android_jni::set_activity_context`).
    ///
    /// The legacy `jvm` raw pointer is not part of the readiness signal: the attach
    /// entry point takes `&self` and cannot populate it, so requiring it made
    /// readiness depend on a field nobody could set. The bridge's own readiness is
    /// authoritative.
    pub fn jni_available(&self) -> bool {
        let _ = &self.jvm;
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::native_view_creation_ready()
        }
        #[cfg(not(feature = "android-jni"))]
        {
            false
        }
    }

    /// Serialize all widget state for parity/regression testing.
    ///
    /// Mirrors the `BackendState` serde gate exactly: the state type only derives
    /// `Serialize` under `serde` and outside the alloc-free `mini`/`embedded`
    /// profiles, so the method must not exist where the bound cannot hold.
    #[cfg(all(feature = "serde_json", feature = "serde", widgets_unstripped))]
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

    /// `insert_widget` is the state allocator the surviving host capabilities use.
    ///
    /// It used to be exercised with `AndroidHandleKind::Button`, a control the host
    /// no longer builds; the window is the kind that still exists, so it is what the
    /// allocator is verified against.
    #[test]
    fn test_android_platform_insert_widget() {
        let platform = AndroidPlatform::new();
        let id = platform.insert_widget(AndroidHandleKind::Window, "Main", 10, 20, 100, 30);
        assert_ne!(id, 0);
        assert_eq!(platform.kind_of(id), Some(AndroidHandleKind::Window));
    }
}
