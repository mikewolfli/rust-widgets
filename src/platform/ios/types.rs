// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! iOS mobile platform types and state container.
//!
//! This module provides the state-backed platform implementation for iOS: every
//! widget is recorded in `BackendState<IosHandleKind>`, and platform contract
//! methods translate between the Rust API and state mutations.
//!
//! # BLUE15: the state model is the whole widget story
//!
//! Widget creation used to instantiate a real UIKit control per logical widget
//! and mirror the state into it. Under the self-drawn strategy the library paints
//! every `WidgetKind`, so the host owes a **window** and a **drawing surface** and
//! nothing per-kind (rules #55/#56). The state record is therefore not a shadow of
//! a UIKit object any more; it is the authority, and it stays because it is what
//! the host's window, menu and event plumbing is expressed in.

use crate::platform::state::BackendState;
#[cfg(all(feature = "serde", widgets_unstripped))]
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// iOS-specific handle type discriminator.
///
/// # Why this is small
///
/// It used to enumerate every `WidgetKind` the host could build a `UIView` for,
/// because the host owned a real control per kind. The library paints every
/// `WidgetKind` now, so the host owns exactly two things a widget cannot: the
/// **window** and the **menu data model** (iOS ships no native menu bar, so the
/// menu tree is in-process bookkeeping the caller drives through
/// `inject_menu_trigger`). Everything else a `create_*` used to record here is
/// state the widget already holds, reached through [`crate::widget::runtime`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub(crate) enum IosHandleKind {
    /// Top-level window.
    Window,
    /// Root menu bar container.
    MenuBar,
    /// Hierarchical menu node.
    Menu,
    /// Actionable menu leaf item.
    MenuItem,
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
/// The widget state machine is platform-independent, so it compiles on every
/// host and its unit tests stay executable there. The one UIKit touch point —
/// the window the library paints into — lives in the `native` sub-module behind
/// `#[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]`.
///
/// All host state is stored in `BackendState<IosHandleKind>`, and platform contract
/// methods translate between the Rust API and state mutations. Per-control list and
/// combo storage used to live here as two maps mirroring what the controls held;
/// they are gone with the controls, because a duplicate of a widget's own state can
/// disagree with it (BLUE15 §10.3).
pub struct IosMobilePlatform {
    /// Internal state for all widgets and handles.
    pub(crate) state: BackendState<IosHandleKind>,
    /// Menu state for menu bar/menu/menu items.
    pub(crate) menus: Mutex<IosMenuState>,
    /// Runtime state for init/run/quit.
    pub(crate) runtime: IosRuntimeState,
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
    #[cfg(all(feature = "serde_json", feature = "serde", widgets_unstripped))]
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
}

crate::impl_default_via_new!(IosMobilePlatform);

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
