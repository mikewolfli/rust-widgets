// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Windows platform types, structs, enums, and traits.

use super::notify;
use crate::platform::state::BackendState;
use crate::platform::{Platform, WidgetTriggerEvent, WidgetTriggerKind};

pub use crate::platform::windows_notify::WindowsHandleKind;

/// The window procedure Win32 calls for the class this backend registers.
///
/// Named `wnd_proc`, not `rw_wnd_proc`: the `rw_` prefix belongs to the C ABI
/// boundary (`src/bindings/`), where a flat global namespace makes it necessary.
/// This function is an internal Win32 callback reached only through a class
/// registration and a `wnd_class.lpfnWndProc` assignment, so a prefix borrowed
/// from a different layer would suggest an ABI export that does not exist.
/// `tools/check_rw_prefix_is_abi_only.sh` enforces the boundary.
#[cfg(target_os = "windows")]
pub(crate) unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use winapi::um::winuser::NMHDR;
    use winapi::um::winuser::{
        DefWindowProcW, GetClientRect, GetDlgCtrlID, PostQuitMessage, WM_COMMAND, WM_DESTROY,
        WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSELEAVE,
        WM_MOUSEMOVE, WM_NOTIFY, WM_PAINT, WM_SIZE,
    };
    match msg {
        // The window's own painting: every control the window owns, drawn as one frame.
        //
        // # Why the top-level window paints anything at all
        //
        // The library paints every `WidgetKind` itself (`control_backend`'s one mechanism),
        // so most controls have **no native control of their own** to draw them. A mounted
        // surface gets a child `HWND` of the canvas class, but the ordinary children —
        // buttons, check boxes, labels — get none, and nothing else painted them: the window
        // showed its chrome over an empty client area, i.e. a blank white board.
        //
        // This arm is the Windows half of the window-level painter the Linux backend already
        // has; both call `render_frame_tree`, which walks the window's child list.
        WM_PAINT => {
            paint_window_tree(hwnd);
            0
        }
        // Painting covers the whole client area, so let it erase too. Returning non-zero
        // skips the background fill and avoids the single white frame a fresh expose would
        // otherwise flash before the tree is drawn (the canvas procedure does the same).
        WM_ERASEBKGND => 1,
        // ── Input ───────────────────────────────────────────────────────────
        //
        // The window's tree painter is also its input surface.
        //
        // # Why the window procedure routes pointer events at all
        //
        // Painting a window's controls means the *window* is where the pointer lands:
        // an ordinary control has no native `HWND` of its own to receive a click, so
        // these messages arrive here and nowhere else. The canvas procedure already
        // does this for a mounted surface; without the same arms on the window, a demo
        // whose controls were created through `create_*` had controls that were visible
        // with correct geometry and callbacks that **never ran** — nothing looked wrong,
        // which is what made it worth its own comment.
        //
        // The coordinates Win32 reports are relative to this window's client area, and
        // controls are positioned in that same space (`frame_origin` is `(0, 0)` for a
        // window), so no origin offset is applied — unlike the canvas, which sits at an
        // offset inside its parent.
        WM_MOUSEMOVE => {
            forward_window_mouse(hwnd, lparam, MousePhase::Drag);
            0
        }
        WM_LBUTTONDOWN => {
            eprintln!(
                "[CLICK] WM_LBUTTONDOWN hwnd={hwnd:?} window_widget={:?}",
                window_widget_for(hwnd)
            );
            forward_window_mouse(hwnd, lparam, MousePhase::Press);
            0
        }
        WM_LBUTTONUP => {
            forward_window_mouse(hwnd, lparam, MousePhase::Release);
            0
        }
        // A hover highlight has to be cleared when the pointer leaves, or it sticks.
        WM_MOUSELEAVE => {
            if window_widget_for(hwnd).is_some() {
                // The point is only carried into the `MouseLeave` the router delivers, and
                // the pointer has already left the client area, so zero is the honest
                // coordinate rather than a stale one.
                crate::widget::runtime::clear_hover(crate::core::Point::new(0, 0));
                invalidate_window(hwnd);
            }
            0
        }
        // Keys go to whatever the pointer router focused, so typing reaches a field the
        // user clicked rather than always the window. Tab is forwarded too, which is how
        // focus moves between controls.
        WM_KEYDOWN => {
            if let Some(window_id) = window_widget_for(hwnd) {
                let target = crate::widget::runtime::focused_widget().unwrap_or(window_id);
                let event = crate::event::Event::KeyPress { key: wparam as u32, modifiers: 0 };
                if crate::widget::runtime::dispatch_event(target, &event) {
                    invalidate_window(hwnd);
                }
            }
            0
        }
        // The user resized the window (or the window manager did). Report the new client
        // size so the host can re-run its layout: without this a window resized by the
        // user kept every child at the geometry it had for the previous size, because
        // nothing else tells the library the window changed.
        WM_SIZE => {
            if let Some(platform) = notify::active_windows_platform() {
                if let Some(widget_id) = platform.widget_id_by_native_handle(hwnd) {
                    let mut rect =
                        winapi::shared::windef::RECT { left: 0, top: 0, right: 0, bottom: 0 };
                    // SAFETY: `hwnd` is the window this procedure was called for, and
                    // Win32 fills the RECT we hand it. A failure leaves the zeros, which
                    // are rejected below rather than reported as a size.
                    if unsafe { GetClientRect(hwnd, &mut rect) } != 0 {
                        let width = (rect.right - rect.left).max(0) as u32;
                        let height = (rect.bottom - rect.top).max(0) as u32;
                        if width > 0 && height > 0 {
                            crate::queue_resize_trigger(widget_id, width, height);
                        }
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_COMMAND => {
            let command_id = (wparam & 0xFFFF) as u32;
            let notify_code = ((wparam >> 16) & 0xFFFF) as u32;
            if let Some(platform) = notify::active_windows_platform() {
                if let Ok(map) = platform.menu_state.menu_command_to_item.lock() {
                    if let Some(item_id) = map.get(&command_id).copied() {
                        if let Ok(mut queue) = platform.menu_state.pending_menu_events.lock() {
                            queue.push_back(WidgetTriggerEvent {
                                widget_id: item_id,
                                kind: WidgetTriggerKind::Clicked,
                            });
                        }
                        return 0;
                    }
                }
                if let Ok(map) = platform.menu_state.control_command_to_widget.lock() {
                    if let Some(widget_id) = map.get(&command_id).copied() {
                        if notify::enqueue_control_notify_event(platform, widget_id, notify_code) {
                            return 0;
                        }
                    }
                }
                if lparam != 0 {
                    let hwnd_from = lparam as HWND;
                    let fallback_command_id = unsafe { GetDlgCtrlID(hwnd_from) } as u32;
                    if fallback_command_id != 0 {
                        if let Ok(map) = platform.menu_state.control_command_to_widget.lock() {
                            if let Some(widget_id) = map.get(&fallback_command_id).copied() {
                                if notify::enqueue_control_notify_event(
                                    platform,
                                    widget_id,
                                    notify_code,
                                ) {
                                    return 0;
                                }
                            }
                        }
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NOTIFY => {
            if let Some(platform) = notify::active_windows_platform() {
                let hdr = lparam as *const NMHDR;
                if !hdr.is_null() {
                    let hwnd_from = unsafe { (*hdr).hwndFrom };
                    let notify_code = unsafe { (*hdr).code };
                    if let Some(widget_id) = platform.widget_id_by_native_handle(hwnd_from) {
                        if let Some(kind) =
                            platform.state.kind_of(widget_id).and_then(|widget_kind| {
                                notify::notify_kind_for_widget(widget_kind, notify_code)
                            })
                        {
                            if let Ok(mut events) = platform.menu_state.pending_widget_events.lock()
                            {
                                events.push_back(WidgetTriggerEvent { widget_id, kind });
                            }
                            return 0;
                        }
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        // Win32 has no `SetWindowMinSize` API: the minimum is enforced by writing
        // `ptMinTrackSize` into the MINMAXINFO the system passes before a resize or
        // maximise. Returning 0 here after filling it in tells the system the
        // constraint was applied.
        WM_GETMINMAXINFO => {
            if let Some(platform) = notify::active_windows_platform() {
                if let Some(widget_id) = platform.widget_id_by_native_handle(hwnd) {
                    if let Some((min_w, min_h)) = platform.state.window_min_size(widget_id) {
                        if !lparam_is_null(lparam) {
                            let info = lparam as *mut winapi::um::winuser::MINMAXINFO;
                            // SAFETY: Win32 passes a valid MINMAXINFO pointer in
                            // lParam for WM_GETMINMAXINFO for the duration of the
                            // call; we only write into it.
                            unsafe {
                                (*info).ptMinTrackSize.x = min_w as i32;
                                (*info).ptMinTrackSize.y = min_h as i32;
                            }
                            return 0;
                        }
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Paints a window's whole widget tree into its client area.
///
/// # Why the top-level window paints at all
///
/// Since 2.0 the library paints **every** `WidgetKind` itself, so a control created with
/// `create_button` / `create_checkbox` / … has no native Win32 control behind it. The only
/// Windows painter used to be the canvas child window that `mount_surface` creates, so a
/// window whose children were created through the ordinary `create_*` path — every control
/// in `demo/control` and `demo/finance` — had nothing drawing them and showed a blank
/// client area.
///
/// This is the Windows counterpart of the window-content painter the Linux backend has: a
/// mounted surface (a child `HWND`) still paints itself, and the tree painter here covers
/// everything that has no surface of its own. A window may therefore mix both kinds.
///
/// # The id translation, and why both hops are needed
///
/// Win32 hands this procedure **its own** `HWND`. Resolving what to draw therefore takes
/// two steps, in this order:
///
/// 1. `HWND` → the id the platform knows it by (`widget_id_by_native_handle`);
/// 2. that id → the widget-registry id (`widget_id_for_host_window`), which is the id the
///    tree lives under.
///
/// Skipping a hop would look up an id that addresses no widget and draw nothing — a
/// silently blank window, which is the failure this function exists to remove.
#[cfg(target_os = "windows")]
unsafe fn paint_window_tree(hwnd: HWND) {
    use winapi::um::winuser::{BeginPaint, EndPaint, PAINTSTRUCT};

    let mut paint: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut paint);

    match super::canvas::client_size(hwnd) {
        Some((width, height)) => {
            // TEMP DIAGNOSTIC: time the root draw and the child walk separately.
            if let Some(widget_id) = paint_target_for(hwnd) {
                let t0 = std::time::Instant::now();
                let kids = crate::widget::runtime::children_of(widget_id);
                let kids_us = t0.elapsed().as_micros();
                let t1 = std::time::Instant::now();
                let frame = crate::widget::runtime::render_frame_tree(
                    widget_id,
                    crate::core::Size::new(width, height),
                    crate::core::Color::WHITE,
                );
                let render_us = t1.elapsed().as_micros();
                eprintln!(
                    "[PAINT] kids_lookup={kids_us}us render={render_us}us kids={} per_kid={}us",
                    kids.len(),
                    render_us / (kids.len().max(1) as u128)
                );
                if let Some(frame) = frame {
                    let t2 = std::time::Instant::now();
                    super::canvas::blit_frame(hdc, width, height, &frame);
                    eprintln!("[PAINT]   blit={}us", t2.elapsed().as_micros());
                }
            }
        }
        None => {}
    }

    EndPaint(hwnd, &paint);
}

/// The widget-registry id of the window whose client area `hwnd` is.
///
/// Extracted from [`paint_target_for`] because the input arms need the same resolution:
/// a `WM_MOUSEMOVE` and a `WM_PAINT` for the same window must agree on which widget tree
/// they name, and two copies of a two-hop translation is how they would stop agreeing.
///
/// `hwnd` → platform id → widget-registry id, in that order. Win32 hands every callback
/// the `HWND`, the backend binds `HWND`s against **platform** ids, and the tree is keyed by
/// **registry** id, so both hops are required.
#[cfg(target_os = "windows")]
unsafe fn window_widget_for(hwnd: HWND) -> Option<u64> {
    crate::widget::runtime::widget_id_for_host_window(widget_id_by_native_handle(hwnd)?)
}

/// The platform's own id for `hwnd`, or `None` when this backend did not create it.
#[cfg(target_os = "windows")]
unsafe fn widget_id_by_native_handle(hwnd: HWND) -> Option<u64> {
    notify::active_windows_platform()?.widget_id_by_native_handle(hwnd)
}

/// Marks `hwnd`'s whole client area as needing a repaint.
#[cfg(target_os = "windows")]
unsafe fn invalidate_window(hwnd: HWND) {
    // SAFETY: a null `RECT` invalidates the entire client area, which is intended.
    winapi::um::winuser::InvalidateRect(hwnd, std::ptr::null(), 0);
}

#[cfg(target_os = "windows")]
use crate::platform::MousePhase;

/// Translates a Win32 mouse message on a **window** into a widget event and delivers it.
///
/// # Coordinate space, and why the window is not offset
///
/// The low and high words of `lparam` hold client-area coordinates, and a child control's
/// geometry is in that same window-relative space, so the point is used as Win32 reports
/// it. The **window widget itself** carries its screen position (`demo/control`'s window is
/// at `x: 100, y: 100`), because that is the geometry it was created with and a window has
/// no parent to be relative to.
///
/// Those are two different spaces in one tree, which is why [`crate::widget::runtime::widget_at`]
/// treats a root as a container rather than testing it against its own bounds: the root's
/// rectangle describes where the OS put the window, and the point describes where the
/// pointer is inside it. Asking the root to contain a client point is the mismatch that
/// made every click miss.
///
/// # Routing
///
/// `dispatch_pointer_event` hit-tests the point against the window's child tree, so a click
/// on a button reaches that button. Child geometry is window-relative, which is the space
/// `position` is already in.
#[cfg(target_os = "windows")]
unsafe fn forward_window_mouse(hwnd: HWND, lparam: isize, phase: MousePhase) {
    use crate::core::Point;
    let Some(window_id) = window_widget_for(hwnd) else {
        return;
    };
    let client_x = (lparam & 0xFFFF) as u16 as i16 as i32;
    let client_y = ((lparam >> 16) & 0xFFFF) as u16 as i16 as i32;
    // Client coordinates, with **no** window offset.
    //
    // A window's own geometry holds the `x`/`y` the caller asked the operating system
    // for, while its children are placed in client coordinates — two different spaces, as
    // `widget::runtime::frame_origin` documents for the painter. A child therefore sits at
    // exactly the client point Win32 reports, and the root is the widget whose bounds do
    // not describe that space at all.
    let position = Point::new(client_x, client_y);
    let event = match phase {
        MousePhase::Press => crate::event::Event::mouse_press_with(
            position.x,
            position.y,
            1,
            crate::platform::windows::canvas::current_modifiers(),
        ),
        MousePhase::Release => crate::event::Event::MouseRelease { pos: position, button: 1 },
        MousePhase::Drag => crate::event::Event::MouseMove { pos: position },
    };
    let delivered = crate::widget::runtime::dispatch_pointer_event(window_id, &event, position);
    if matches!(phase, MousePhase::Drag) {
        // The request is consumed by the event it produces, so it must be re-issued on
        // every move — without it the leave message never arrives and hover sticks.
        let mut track: winapi::um::winuser::TRACKMOUSEEVENT = std::mem::zeroed();
        track.cbSize = std::mem::size_of::<winapi::um::winuser::TRACKMOUSEEVENT>() as u32;
        track.dwFlags = winapi::um::winuser::TME_LEAVE;
        track.hwndTrack = hwnd;
        winapi::um::winuser::TrackMouseEvent(&mut track);
    }
    if delivered {
        if matches!(phase, MousePhase::Press) {
            // A click can move focus to a nested control; give the window the keyboard so
            // subsequent keys are delivered here.
            winapi::um::winuser::SetFocus(hwnd);
        }
        invalidate_window(hwnd);
    }
}

/// Resolves the widget-registry id whose tree this window should paint.
///
/// Both hops are done here rather than inline so the resolution has one definition, and
/// so a failure can say *which* hop failed: an unknown handle and a known handle with no
/// widget behind it are different faults, and collapsing them into one `None` is how a
/// missing association goes unnoticed.
#[cfg(target_os = "windows")]
unsafe fn paint_target_for(hwnd: HWND) -> Option<u64> {
    let Some(host) = widget_id_by_native_handle(hwnd) else {
        log::error!("[windows] WM_PAINT for hwnd {hwnd:?}, which has no platform id bound to it");
        return None;
    };
    // `host` is the **platform** id `create_window` returned, so the registry lookup is the
    // second hop and `host` is its input. Feeding it to `widget_id_for_host_window` is not
    // the same question: that function's argument is a widget-registry id.
    match crate::widget::runtime::widget_id_for_host_window(host) {
        Some(id) => Some(id),
        None => {
            log::error!(
                "[windows] window hwnd {hwnd:?} (platform id {host}) has no widget-registry \
                 association yet; nothing to paint"
            );
            None
        }
    }
}

/// Whether the message's `lParam` is a null pointer.
///
/// Kept as a tiny helper so the `WM_GETMINMAXINFO` arm reads clearly and the
/// cast-and-compare happens in exactly one place.
#[cfg(target_os = "windows")]
fn lparam_is_null(lparam: isize) -> bool {
    lparam == 0
}
#[cfg(target_os = "windows")]
impl WindowsPlatform {
    /// Encodes `s` as a NUL-terminated UTF-16 buffer.
    ///
    /// This is the form every Win32 `*W` entry point expects (`CreateWindowExW`,
    /// `SetWindowTextW`, `MessageBoxW`, …). `String`/`&str` cannot be passed
    /// directly because Win32 wide strings are neither length-prefixed nor
    /// guarantee-terminated by Rust.
    ///
    /// `Vec` comes from `crate::compat` so this compiles under the `mini`
    /// profile, which is `no_std` and has no `std` prelude to resolve `Vec` from.
    pub fn to_wide(s: &str) -> crate::compat::Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }
    /// Returns the native window handle recorded for `id`, if any.
    ///
    /// Only ids that were bound via [`Self::bind_native_handle`] are present; a
    /// widget id that the library created without a host window has no handle and
    /// yields `None`. A poisoned handle map is logged and treated as `None` rather
    /// than panicking.
    pub fn get_native_handle(&self, id: u64) -> Option<HWND> {
        #[cfg(target_os = "windows")]
        {
            match self.menu_state.handles.lock() {
                Ok(handles) => handles.get(&id).map(|&h| h as HWND),
                Err(_) => {
                    log::error!(
                        "[rust_widgets][windows] get_native_handle: handles mutex poisoned"
                    );
                    None
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
    /// Records `hwnd` as the native window for `id`.
    ///
    /// Also registers the handle with the accessibility bridge, so UIAutomation
    /// notifications can be raised against the real window. Binding an id twice
    /// replaces the previous handle. A poisoned map is ignored: the handle is
    /// dropped rather than panicking inside a message-pump callback.
    pub fn bind_native_handle(&self, id: u64, hwnd: HWND) {
        #[cfg(target_os = "windows")]
        {
            if let Ok(mut handles) = self.menu_state.handles.lock() {
                handles.insert(id, hwnd as usize);
            } else {
                // Handle lock error explicitly
            }
            self.a11y_bridge.register_handle(id, hwnd as usize);
        }
    }
    #[cfg(target_os = "windows")]
    /// # Safety
    ///
    /// Caller must ensure that `hwnd` is a valid native window handle
    /// and that it remains valid for the duration of this call.
    /// Modifying the window's identifier via `SetWindowLongPtrW` can
    /// affect window procedure behavior; callers should ensure this
    /// is done only for windows owned by this platform adapter.
    pub unsafe fn bind_control_command(&self, widget_id: u64, hwnd: HWND) {
        use winapi::um::winuser::{SetWindowLongPtrW, GWLP_ID};
        let command_id = self.menu_state.next_command_id.fetch_add(1, Ordering::SeqCst) as u32;
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_ID, command_id as isize);
        }
        if let Ok(mut map) = self.menu_state.control_command_to_widget.lock() {
            map.insert(command_id, widget_id);
        }
    }
    /// Returns the widget id whose native handle is `hwnd`.
    ///
    /// The reverse of [`Self::get_native_handle`], used to map a `WM_COMMAND`
    /// notification back to the library widget that owns it. Linear in the number
    /// of bound handles. `None` when no widget owns that handle, or when the handle
    /// map is poisoned (logged, not panicked).
    #[cfg(target_os = "windows")]
    pub fn widget_id_by_native_handle(&self, hwnd: HWND) -> Option<u64> {
        match self.menu_state.handles.lock() {
            Ok(handles) => handles
                .iter()
                .find_map(|(widget_id, native)| ((*native as HWND) == hwnd).then_some(*widget_id)),
            Err(_) => {
                log::error!(
                    "[rust_widgets][windows] widget_id_by_native_handle: handles mutex poisoned"
                );
                None
            }
        }
    }
}
/// Extension trait for downcasting `dyn Platform` to concrete platform types.
pub trait PlatformDowncast {
    /// Downcasts to `T`, returning `None` when the backend is a different type.
    fn downcast_ref<T: 'static>(&self) -> Option<&T>;
}
impl PlatformDowncast for dyn Platform {
    fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}
#[cfg(target_os = "windows")]
use winapi::shared::windef::HWND;
#[cfg(not(target_os = "windows"))]
type HWND = *mut std::ffi::c_void;
// Windows backend shell.
#[cfg(target_os = "windows")]
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(target_os = "windows")]
use std::sync::Mutex;
/// Windows platform backend struct definition
pub struct WindowsPlatform {
    /// Host-side widget/surface state shared with every backend.
    pub state: BackendState<WindowsHandleKind>,
    /// Whether [`Platform::init`] has already run.
    ///
    /// Read at the top of `init` to make repeat entry a no-op: `init` registers the
    /// platform and calls `InitCommonControls`, and doing either twice is wasteful at
    /// best.
    pub runtime_initialized: AtomicBool,
    /// Whether the run loop is currently active.
    pub runtime_running: AtomicBool,
    /// Menu and command-routing state (Win32 only).
    #[cfg(target_os = "windows")]
    pub menu_state: Win32MenuState,
    // Removed handle_state: Win32HandleState, as Win32HandleState is not defined in state.rs
    /// Platform IME bridge for text input method integration (Windows TSF).
    /// Uses `ime_windows::WindowsImeBridge` (real state machine, no fake COM vtables).
    pub ime_bridge: crate::platform::ime_windows::WindowsImeBridge,
    /// Platform rich clipboard backend.
    pub clipboard: crate::platform::clipboard_stubs::windows::WindowsClipboard,
    /// Platform accessibility bridge for UIAutomation notifications.
    #[cfg(target_os = "windows")]
    pub a11y_bridge: crate::platform::accessibility::windows::WindowsAccessibilityBridge,
    #[cfg(not(target_os = "windows"))]
    pub a11y_bridge: (),
}
/// Win32 menu state holder.
/// Reserved for Windows platform menu integration — stores HWND handles and
/// command-to-widget mappings. Only compiled on Windows targets.
#[cfg(target_os = "windows")]
pub struct Win32MenuState {
    // SAFETY: HWND is only used on the main thread, and Win32MenuState is not shared across threads in this context.
    pub(crate) handles: Mutex<HashMap<u64, usize>>,
    pub(crate) menu_command_to_item: Mutex<HashMap<u32, u64>>,
    pub(crate) control_command_to_widget: Mutex<HashMap<u32, u64>>,
    pub(crate) pending_menu_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    pub(crate) pending_widget_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    pub(crate) next_command_id: AtomicU64,
}
#[cfg(target_os = "windows")]
impl Win32MenuState {
    fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            menu_command_to_item: Mutex::new(HashMap::new()),
            control_command_to_widget: Mutex::new(HashMap::new()),
            pending_menu_events: Mutex::new(VecDeque::new()),
            pending_widget_events: Mutex::new(VecDeque::new()),
            next_command_id: AtomicU64::new(1000),
        }
    }
}
#[cfg(target_os = "windows")]
// Extension trait for native Win32 Slider (Trackbar) integration
impl WindowsPlatform {
    /// Creates a backend with no windows, no menus and no bound handles.
    pub fn new() -> Self {
        WindowsPlatform {
            state: BackendState::new(),
            runtime_initialized: AtomicBool::new(false),
            runtime_running: AtomicBool::new(false),
            #[cfg(target_os = "windows")]
            menu_state: Win32MenuState::new(),
            ime_bridge: crate::platform::ime_windows::WindowsImeBridge::new(),
            clipboard: crate::platform::clipboard_stubs::windows::WindowsClipboard,
            #[cfg(target_os = "windows")]
            a11y_bridge: crate::platform::accessibility::windows::WindowsAccessibilityBridge::new(),
            #[cfg(not(target_os = "windows"))]
            a11y_bridge: (),
        }
    }
}

crate::impl_default_via_new!(WindowsPlatform);
