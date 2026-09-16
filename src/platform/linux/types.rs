// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Linux backend shell.
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::compat::HashMap;
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::compat::Mutex;
use crate::platform::state::BackendState;
use std::sync::atomic::AtomicBool;
/// Logical handle kinds that survive the self-drawn widget strategy.
///
/// # BLUE15: the host no longer builds controls
///
/// This used to enumerate every logical control (`Button`, `Label`, `ListBox`,
/// `MenuBar`, ...). Under the self-drawn strategy the host owes the widget layer a
/// window and a drawing surface, and the library paints every `WidgetKind`, so a
/// per-kind `create_*` has no host object to map onto (BLUE15 #56). The window is
/// the one primitive GTK still supplies; the menu is an in-process model the host
/// materialises through its own widgets.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LinuxHandleKind {
    /// Top-level GTK window handed to the library as a drawing surface.
    Window,
}
/// Runtime lifecycle state for Linux backend main loop fallback.
pub(crate) struct LinuxRuntimeState {
    pub(crate) initialized: AtomicBool,
    pub(crate) running: AtomicBool,
}
impl LinuxRuntimeState {
    pub(crate) fn new() -> Self {
        Self { initialized: AtomicBool::new(false), running: AtomicBool::new(false) }
    }
}
/// Linux desktop platform adapter.
pub struct LinuxPlatform {
    pub(crate) state: BackendState<LinuxHandleKind>,
    pub(crate) runtime: LinuxRuntimeState,
    #[cfg(all(target_os = "linux", feature = "gtk-native"))]
    pub(crate) native: Mutex<LinuxNativeState>,
    /// Platform IME bridge for text input method integration (Linux only).
    #[cfg(target_os = "linux")]
    pub(crate) ime_bridge: crate::platform::ime_linux::LinuxImeBridge,
}
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
#[derive(Default)]
pub(crate) struct LinuxNativeState {
    /// Native GTK windows indexed by logical widget id.
    pub(crate) windows: HashMap<u64, gtk::Window>,
    /// Root vertical containers hosting menu bar and content area.
    pub(crate) root_boxes: HashMap<u64, gtk::Box>,
    /// Absolute-position container for child controls.
    pub(crate) content_fixed: HashMap<u64, gtk::Fixed>,
    /// Generic widget registry for visibility/text/enabled operations.
    pub(crate) widgets: HashMap<u64, gtk::Widget>,
    /// Native `DrawingArea`s hosting self-drawn widgets, indexed by the widget
    /// registry id they paint (see `linux/canvas.rs`).
    ///
    /// Gated exactly like `canvas.rs`, which is its only reader and writer: a
    /// stripped build (`mini`/`embedded`) has no `widget::runtime` to paint from,
    /// so there is nothing to host here.
    #[cfg(widgets_unstripped)]
    pub(crate) canvases: HashMap<u64, gtk::DrawingArea>,
}

#[cfg(all(target_os = "linux", feature = "gtk-native"))]
unsafe impl Send for LinuxNativeState {}

// SAFETY: `LinuxPlatform` is only ever driven from the UI thread. The GTK
// widgets in `native` (`Mutex<LinuxNativeState>`) are `!Send + !Sync`, and GTK
// itself requires all calls to happen on the thread that called `gtk::init`.
//
// `Send` is required because the platform handle is stored in the crate's
// process-global registry; it does NOT permit concurrent GTK access, because
// every native entry point re-checks `gtk::is_initialized_main_thread()` before
// touching GTK (see `platform_impl.rs`).
//
// `Sync` is deliberately NOT implemented: nothing requires it, and the `gtk`
// types are `!Sync`, so a hand-written `unsafe impl Sync` would be an
// unnecessary promise that `&LinuxPlatform` is safe to share across threads.

#[cfg(all(target_os = "linux", feature = "gtk-native"))]
unsafe impl Send for LinuxPlatform {}

impl LinuxPlatform {
    /// Creates a new Linux platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            runtime: LinuxRuntimeState::new(),
            #[cfg(all(target_os = "linux", feature = "gtk-native"))]
            native: Mutex::new(LinuxNativeState::default()),
            #[cfg(target_os = "linux")]
            ime_bridge: crate::platform::ime_linux::LinuxImeBridge::new(),
        }
    }
}
crate::impl_default_via_new!(LinuxPlatform);
impl LinuxPlatform {
    /// Insert and initialize one widget state record.
    pub(crate) fn insert_widget(
        &self,
        kind: LinuxHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.state.create_widget(kind, text, x, y, width, height)
    }
}
