// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Wayland backend platform types.
//!
//! This module defines the Wayland platform's widget handle kind enum, the
//! in-process menu state, and the main `WaylandPlatform` adapter.
//!
//! # BLUE15: no per-kind control state
//!
//! `WaylandHandleKind` used to carry one variant per logical control — `Button`,
//! `Label`, `ComboBox`, every dialog kind and so on — because every
//! `Platform::create_*` override classified the widget it had just recorded. Those
//! overrides are gone: the library paints every `WidgetKind`, and `xdg_shell` has
//! no protocol object for a button or a dialog, so nothing classified a control's
//! kind and no per-control state was ever read back. Keeping the variants (or the
//! list storage that fed them) would leave dead state that the next reader has to
//! disprove; rule #59 says delete means delete.
//!
//! The variants that remain are the ones the surviving code actually names: the
//! window, and the three kinds of the in-process menu model.
//!
//! ## Menu semantics (Wayland)
//!
//! Wayland defines no menu protocol, so `MenuBar`/`Menu`/`MenuItem` are not host
//! controls. They are in-process data with kind-constrained parents and injectable
//! trigger events, matching the iOS backend's `IosHandleKind` menu variants and
//! the contract stated in `crate::platform::types`.

use crate::platform::state::BackendState;
use crate::platform::WidgetTriggerEvent;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

/// Platform-specific widget handle kind for the Wayland backend.
///
/// `Window` names the host capability this backend owns (an `xdg_toplevel`);
/// `MenuBar`/`Menu`/`MenuItem` name the nodes of the in-process menu data model.
/// Every other `WidgetKind` is painted by `src/widget/` and needs no host-side
/// classification, so no variant exists for it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum WaylandHandleKind {
    /// Top-level window (wl_surface / xdg_surface / xdg_toplevel).
    Window,
    /// Root of the in-process menu model; owned by a window.
    ///
    /// Not a compositor object: Wayland has no menu protocol.
    MenuBar,
    /// Hierarchical node of the in-process menu model; owned by a menu bar or
    /// another menu.
    Menu,
    /// Leaf of the in-process menu model; carries text plus the shortcut's
    /// display form. Owned by a menu.
    MenuItem,
}

/// Runtime state for the in-process menu data model.
#[derive(Default)]
pub(crate) struct WaylandMenuState {
    /// Maps window id to attached menu bar id.
    pub(crate) attached_menu_bar: HashMap<u64, u64>,
    /// Maps parent menu id to child menu item ids.
    pub(crate) menu_children: HashMap<u64, Vec<u64>>,
    /// FIFO queue for menu trigger events.
    pub(crate) pending_menu_events: VecDeque<u64>,
    /// FIFO queue for typed widget trigger events.
    pub(crate) pending_widget_events: VecDeque<WidgetTriggerEvent>,
}

/// Runtime lifecycle state for the Wayland backend.
pub(crate) struct WaylandRuntimeState {
    pub(crate) initialized: AtomicBool,
    pub(crate) running: AtomicBool,
}

impl WaylandRuntimeState {
    pub(crate) fn new() -> Self {
        Self { initialized: AtomicBool::new(false), running: AtomicBool::new(false) }
    }
}

/// Wayland desktop platform adapter.
///
/// Provides the `Platform` trait implementation backed by
/// `BackendState<WaylandHandleKind>` for windows and by the native Wayland
/// session for the actual surface. Controls are painted by the library, so this
/// adapter holds no per-control state and no toolkit handles.
pub struct WaylandPlatform {
    pub(crate) state: BackendState<WaylandHandleKind>,
    pub(crate) menus: Mutex<WaylandMenuState>,
    pub(crate) runtime: WaylandRuntimeState,
    /// Persistent Wayland session (connection, event queue, and global proxies).
    /// Initialized on first `try_create_native_window` call; used by `run()` for event dispatch.
    #[cfg(all(feature = "wayland-native", target_os = "linux"))]
    pub(crate) native_session:
        Mutex<Option<crate::platform::wayland::platform_impl::WaylandSession>>,
}

impl WaylandPlatform {
    /// Creates a new Wayland platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            menus: Mutex::new(WaylandMenuState::default()),
            runtime: WaylandRuntimeState::new(),
            #[cfg(all(feature = "wayland-native", target_os = "linux"))]
            native_session: Mutex::new(None),
        }
    }
}

impl Default for WaylandPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl WaylandPlatform {
    /// Insert widget state record and return allocated logical id.
    pub(crate) fn insert_widget(
        &self,
        kind: WaylandHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.state.create_widget(kind, text, x, y, width, height)
    }
}
