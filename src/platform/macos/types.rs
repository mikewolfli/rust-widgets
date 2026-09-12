//! macOS platform types, structs, enums, and helper functions.

#![allow(deprecated)] // Cocoa 0.24 fallback; remove when objc2 backend fully replaces cocoa

use crate::core::ObjectId;
use crate::platform::accessibility::macos::MacOSAccessibilityBridge;
use crate::platform::state::BackendState;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
use cocoa::appkit::{NSView, NSWindow, NSWindowStyleMask};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum HandleKind {
    /// Top-level NSWindow.
    Window,
    Button,
    CheckBox,
    RadioButton,
    Label,
    LineEdit,
    Slider,
    ProgressBar,
    ComboBox,
    ListBox,
    Panel,
    MenuBar,
    Menu,
    /// NSMenuItem instance that represents a selectable action.
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
    /// A self-drawn widget mounted into a window (see `macos/canvas.rs`).
    ///
    /// Absent from `mini`/`embedded` because that is where `canvas.rs` — its only
    /// producer — is compiled out.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Canvas,
}
#[derive(Clone, Copy)]
pub(crate) struct CocoaHandle {
    /// Opaque native pointer cast to usize.
    pub(crate) ptr: usize,
    /// Runtime handle kind used for dispatch.
    pub(crate) kind: HandleKind,
}

/// macOS desktop platform adapter.
pub struct MacOSPlatform {
    /// Shared logical widget state split from native handles.
    pub(crate) state: BackendState<HandleKind>,
    /// Logical id -> native handle mapping.
    pub(crate) handles: Mutex<HashMap<ObjectId, CocoaHandle>>,
    /// Combo-box item storage per logical widget id.
    pub(crate) combo_box_items: Mutex<HashMap<ObjectId, Vec<String>>>,
    /// Combo-box selected index per logical widget id.
    pub(crate) combo_box_selection: Mutex<HashMap<ObjectId, Option<usize>>>,
    /// List-box item storage per logical widget id.
    pub(crate) list_box_items: Mutex<HashMap<ObjectId, Vec<String>>>,
    /// List-box selected index per logical widget id.
    pub(crate) list_box_selection: Mutex<HashMap<ObjectId, Option<usize>>>,
    /// Platform IME bridge for text input method integration.
    pub(crate) ime_bridge: crate::platform::ime_macos::MacOsImeBridge,
    /// Platform rich clipboard backend.
    pub(crate) clipboard: crate::platform::clipboard_stubs::macos::MacOsClipboard,
    /// Platform accessibility bridge for NSAccessibility notifications.
    pub(crate) a11y_bridge: MacOSAccessibilityBridge,
    /// Accelerator display text per menu item, so a host can confirm which chord
    /// was actually registered (see `Platform::menu_item_shortcut`).
    pub(crate) menu_item_shortcuts: Mutex<HashMap<ObjectId, String>>,
}

static MENU_EVENTS: OnceLock<Mutex<Vec<u64>>> = OnceLock::new();
static MENU_TARGET: OnceLock<usize> = OnceLock::new();
/// Widget button click events queue.
static WIDGET_EVENTS: OnceLock<Mutex<Vec<WidgetTriggerEvent>>> = OnceLock::new();
pub(crate) fn menu_events() -> &'static Mutex<Vec<u64>> {
    // Shared menu-trigger queue used by Cocoa selector bridge.
    MENU_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}
pub(crate) fn widget_events() -> &'static Mutex<Vec<WidgetTriggerEvent>> {
    // Shared widget-trigger queue used by Cocoa selector bridge.
    WIDGET_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

extern "C" fn on_menu_item(_this: &Object, _cmd: Sel, sender: id) {
    // Selector callback invoked by NSMenuItem actions.
    // SAFETY: This function is called from Objective-C runtime on the main thread.
    // sender is guaranteed by the ObjC runtime to be a valid NSMenuItem instance.
    // msg_send! macros use valid selectors (representedObject, unsignedLongLongValue)
    // that are standard on NSMenuItem. The panic::catch_unwind wrapper prevents
    // unwinding across the FFI boundary, which would be UB.
    let result = std::panic::catch_unwind(|| unsafe {
        if sender == nil {
            return;
        }
        let represented: id = msg_send![sender, representedObject];
        if represented == nil {
            return;
        }
        let item_id: u64 = msg_send![represented, unsignedLongLongValue];
        if item_id != 0 {
            if let Ok(mut events) = menu_events().lock() {
                events.push(item_id);
            }
        }
    });
    if result.is_err() {
        log::error!("[rust_widgets] Panic in on_menu_item handler");
    }
}
extern "C" fn on_button_clicked(_this: &Object, _cmd: Sel, sender: id) {
    // Selector callback invoked by NSButton actions.
    log::info!("[rust_widgets] on_button_clicked: CALLED! sender={:?}", sender);
    // Use catch_unwind to prevent panics from crossing FFI boundary
    let result = std::panic::catch_unwind(|| {
        unsafe {
            if sender == nil {
                log::error!("[rust_widgets] on_button_clicked: sender is nil");
                return;
            }
            let represented: id = msg_send![sender, representedObject];
            log::debug!("[rust_widgets] on_button_clicked: represented={:?}", represented);
            if represented == nil {
                log::error!("[rust_widgets] on_button_clicked: representedObject is nil");
                return;
            }
            let widget_id: u64 = msg_send![represented, unsignedLongLongValue];
            log::debug!("[rust_widgets] on_button_clicked: widget_id = {}", widget_id);
            if widget_id != 0 {
                // Push the event first (radio button handling is secondary)
                if let Ok(mut events) = widget_events().lock() {
                    events.push(WidgetTriggerEvent { widget_id, kind: WidgetTriggerKind::Clicked });
                    log::debug!(
                        "[rust_widgets] on_button_clicked: event pushed, queue size = {}",
                        events.len()
                    );
                } else {
                    log::error!("[rust_widgets] on_button_clicked: failed to lock widget_events");
                }
            }
        }
    });
    if result.is_err() {
        log::error!("[rust_widgets] Panic in on_button_clicked handler");
    }
}
extern "C" fn on_button_clicked_simple(_this: &Object, _cmd: Sel) {
    log::error!("[rust_widgets] on_button_clicked_simple: called");
}
pub(crate) fn menu_target_class() -> *const Class {
    static CLASS: OnceLock<usize> = OnceLock::new();
    (*CLASS.get_or_init(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("RustWidgetsMenuTarget", superclass)
            .expect("failed to declare RustWidgetsMenuTarget");
        unsafe {
            decl.add_method(sel!(onMenuItem:), on_menu_item as extern "C" fn(&Object, Sel, id));
        }
        (decl.register() as *const Class) as usize
    })) as *const Class
}
pub(crate) fn shared_menu_target() -> id {
    let ptr = *MENU_TARGET.get_or_init(|| unsafe {
        let class = menu_target_class();
        let obj: id = msg_send![class, new];
        obj as usize
    });
    ptr as id
}
static BUTTON_TARGET: OnceLock<usize> = OnceLock::new();
pub(crate) fn button_target_class() -> *const Class {
    static CLASS: OnceLock<usize> = OnceLock::new();
    (*CLASS.get_or_init(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("RustWidgetsButtonTarget", superclass)
            .expect("failed to declare RustWidgetsButtonTarget");
        unsafe {
            decl.add_method(
                sel!(onButtonClicked:),
                on_button_clicked as extern "C" fn(&Object, Sel, id),
            );
            // Also add the method with a different selector name for testing
            decl.add_method(
                sel!(buttonClicked:),
                on_button_clicked as extern "C" fn(&Object, Sel, id),
            );
            // Add a simple selector without colon
            decl.add_method(
                sel!(buttonClick),
                on_button_clicked_simple as extern "C" fn(&Object, Sel),
            );
        }
        (decl.register() as *const Class) as usize
    })) as *const Class
}
pub(crate) fn shared_button_target() -> id {
    let ptr = *BUTTON_TARGET.get_or_init(|| unsafe {
        let class = button_target_class();
        log::info!("[rust_widgets] shared_button_target: creating target with class {:?}", class);
        let obj: id = msg_send![class, new];
        log::error!("[rust_widgets] shared_button_target: created obj {:?}", obj);
        // Retain the object to keep it alive
        let _: () = msg_send![obj, retain];
        obj as usize
    });
    log::info!("[rust_widgets] shared_button_target: returning target {:?}", ptr as id);
    ptr as id
}
/// Translates an AppKit `NSEvent` key event into `(key_code, widget_modifiers)`.
///
/// # Safety
///
/// `event` must be a live `NSEvent` instance, or `nil` (yielding `(0, 0)`).
///
/// Gated with `canvas.rs`: it is the only caller, and `mini`/`embedded` have no
/// self-drawn canvas for a key event to arrive through.
#[cfg(all(target_os = "macos", not(any(feature = "mini", feature = "embedded"))))]
pub(crate) unsafe fn translate_key_event(event: id) -> (u32, u32) {
    if event == nil {
        return (0, 0);
    }
    let key_code: u16 = msg_send![event, keyCode];
    let flags: u64 = msg_send![event, modifierFlags];
    map_key_modifiers(flags, key_code)
}

/// Translates an AppKit `NSEvent` key event into `(key_code, widget_modifiers)`.
///
/// Testable core of [`translate_key_event`]: takes the raw AppKit values so the
/// mapping can be asserted without constructing an `NSEvent`.
///
/// # Modifier mapping
///
/// The widget layer uses a small fixed bitfield: shift = 1, control = 2,
/// alt = 4, meta/command = 8 (see `Modifiers::from_event_bits`). This converts
/// AppKit's `NSEventModifierFlag` bits into that layout, so a `KeyPress`
/// arriving through a self-drawn canvas carries the same bits as one arriving
/// through the regular event loop.
///
/// Command **must** be forwarded as bit 3: it is the macOS primary accelerator
/// (`⌘Z`, `⌘S`, ...). Dropping it made every Command chord reach the widget as a
/// bare key press, which is why application shortcuts never fired on macOS.
///
/// # Key code
///
/// AppKit's `keyCode` is a hardware layout code and is returned unchanged; the
/// widget layer's key handling is written against exactly those codes for the
/// non-printing keys (arrows, Enter, Escape, Backspace, Delete, Page Up/Down,
/// Home/End). Printable characters travel as `Event::TextInput` and therefore
/// never depend on this number.
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub(crate) fn map_key_modifiers(appkit_flags: u64, key_code: u16) -> (u32, u32) {
    // AppKit modifier flags are shared with the accelerator parser, which owns
    // their definitions so the two macOS backends cannot disagree on them.
    use super::accelerator::{MOD_COMMAND, MOD_CONTROL, MOD_OPTION, MOD_SHIFT};
    const WIDGET_SHIFT: u32 = 1;
    const WIDGET_CONTROL: u32 = 2;
    const WIDGET_ALT: u32 = 4;
    const WIDGET_META: u32 = 8;
    let mut modifiers = 0u32;
    if appkit_flags & MOD_SHIFT != 0 {
        modifiers |= WIDGET_SHIFT;
    }
    if appkit_flags & MOD_CONTROL != 0 {
        modifiers |= WIDGET_CONTROL;
    }
    if appkit_flags & MOD_OPTION != 0 {
        modifiers |= WIDGET_ALT;
    }
    if appkit_flags & MOD_COMMAND != 0 {
        modifiers |= WIDGET_META;
    }
    (key_code as u32, modifiers)
}

/// Returns `true` when the caller is running on the AppKit main thread.
///
/// AppKit objects (`NSWindow`, `NSView`, `NSApplication`, panels, pasteboard
/// singletons, ...) may only be created or mutated from the main thread; doing
/// otherwise raises an Objective-C exception that Rust cannot catch, which
/// aborts the whole process with
/// `fatal runtime error: Rust cannot catch foreign exceptions`.
///
/// Every native entry point in this legacy backend therefore consults this
/// guard first and falls back to a state-only handle when it returns `false`.
/// This mirrors the `objc2::MainThreadMarker::new()` gating already used by the
/// `macos_objc2` backend and by `create_native_dialog` here.
pub(crate) fn is_main_thread() -> bool {
    // SAFETY: `+[NSThread isMainThread]` is a thread-safe class method that is
    // safe to call from any thread and never raises.
    unsafe { msg_send![class!(NSThread), isMainThread] }
}

impl MacOSPlatform {
    /// Creates a new macOS platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            handles: Mutex::new(HashMap::new()),
            combo_box_items: Mutex::new(HashMap::new()),
            combo_box_selection: Mutex::new(HashMap::new()),
            list_box_items: Mutex::new(HashMap::new()),
            list_box_selection: Mutex::new(HashMap::new()),
            ime_bridge: crate::platform::ime_macos::MacOsImeBridge::new(),
            clipboard: crate::platform::clipboard_stubs::macos::MacOsClipboard,
            a11y_bridge: MacOSAccessibilityBridge::new(),
            menu_item_shortcuts: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for MacOSPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl MacOSPlatform {
    pub(crate) fn make_rect(x: i32, y: i32, width: u32, height: u32) -> NSRect {
        NSRect::new(NSPoint::new(x as f64, y as f64), NSSize::new(width as f64, height as f64))
    }
    pub(crate) fn window_style() -> NSWindowStyleMask {
        NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSClosableWindowMask
            | NSWindowStyleMask::NSResizableWindowMask
            | NSWindowStyleMask::NSMiniaturizableWindowMask
    }
    /// Records a menu item's accelerator text.
    ///
    /// Called from both the native and the state-only path of `menu_add_item` so
    /// an item never disagrees with itself about whether it has a chord.
    pub(crate) fn record_menu_item_shortcut(&self, item_id: ObjectId, shortcut: Option<&str>) {
        let Some(chord) = shortcut.map(str::trim).filter(|s| !s.is_empty()) else {
            return;
        };
        match self.menu_item_shortcuts.lock() {
            Ok(mut shortcuts) => {
                shortcuts.insert(item_id, chord.to_string());
            }
            Err(_) => log::error!("macos menu_item_shortcuts lock poisoned"),
        }
    }

    pub(crate) fn get_handle(&self, widget_id: ObjectId) -> Option<CocoaHandle> {
        let guard = self.handles.lock().expect("macos handle lock poisoned");
        let handle = guard.get(&widget_id).copied();
        if handle.is_none() {
            log::error!(
                "[macos] get_handle: widget_id={} not found in handle registry ({} total handles)",
                widget_id,
                guard.len()
            );
        }
        handle
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn register_handle(
        &self,
        kind: HandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        ptr: usize,
    ) -> ObjectId {
        let id = self.state.create_widget(kind, text, x, y, width, height);
        {
            let mut guard = self.handles.lock().expect("macos handle lock poisoned");
            if guard.contains_key(&id) {
                log::warn!(
                    "[macos] register_handle: overwriting existing handle for widget_id={}",
                    id
                );
            }
            guard.insert(id, CocoaHandle { ptr, kind });
        }
        // Register the native handle with the accessibility bridge.
        self.a11y_bridge.register_handle(id, ptr);
        log::trace!("[macos] register_handle: id={}, kind={:?}, ptr=0x{:x}", id, kind, ptr);
        id
    }
    pub(crate) fn as_id(handle: CocoaHandle) -> id {
        handle.ptr as id
    }
    /// Registers a state-only handle (`ptr == 0`) for the off-main-thread
    /// fallback path, keeping widget ids, text and geometry semantics intact
    /// without ever touching AppKit.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn register_state_only_handle(
        &self,
        kind: HandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.register_handle(kind, text, x, y, width, height, 0)
    }
    pub(crate) fn add_to_parent_window(&self, parent: ObjectId, view: id) {
        if let Some(parent_handle) = self.get_handle(parent) {
            if let HandleKind::Window = parent_handle.kind {
                // A state-only parent handle (ptr == 0, created off the main
                // thread) has no native window to attach to; skip rather than
                // messaging a nil receiver, which the cocoa crate aborts on.
                if parent_handle.ptr == 0 || view == nil {
                    return;
                }
                // SAFETY: parent_handle is validated by get_handle() and confirmed to be a
                // Window kind before entering this block. Self::as_id() converts the stored
                // usize back to a valid ObjC id that was registered by register_handle().
                // NSWindow::contentView() and addSubview_() use selectors proven valid by
                // the cocoa crate. The view parameter is a valid id created earlier in the
                // calling function (e.g., create_button) and is retained by the parent
                // window's view hierarchy after this call.
                unsafe {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(view);
                }
            }
        }
    }
    pub(crate) fn sync_list_box_native(&self, list_box: ObjectId) {
        let Some(handle) = self.get_handle(list_box) else {
            return;
        };
        if !matches!(handle.kind, HandleKind::ListBox) {
            return;
        }
        // Off-main (or a state-only) handle has no native NSTextField to sync;
        // the StringValue update would message a nil receiver.
        if handle.ptr == 0 || !is_main_thread() {
            return;
        }
        let items = match self.list_box_items.lock() {
            Ok(m) => m.get(&list_box).cloned().unwrap_or_default(),
            Err(_) => {
                log::error!("[rust_widgets] sync_list_box_native: list_box_items mutex poisoned");
                Vec::new()
            }
        };
        let selected = match self.list_box_selection.lock() {
            Ok(m) => m.get(&list_box).copied().flatten(),
            Err(_) => {
                log::error!(
                    "[rust_widgets] sync_list_box_native: list_box_selection mutex poisoned"
                );
                None
            }
        };
        let text = if items.is_empty() {
            String::new()
        } else {
            items
                .iter()
                .enumerate()
                .map(
                    |(idx, item)| {
                        if Some(idx) == selected {
                            format!("> {item}")
                        } else {
                            item.clone()
                        }
                    },
                )
                .collect::<Vec<_>>()
                .join("\n")
        };
        // SAFETY: handle has been validated by the kind check above (must be ListBox).
        // Self::as_id(handle) converts the stored usize back to a valid ObjC id that
        // was registered by register_handle(). setStringValue: is a valid selector on
        // NSTextField (used as a list box surrogate). NSString::alloc(nil).init_str()
        // produces a valid NSString - nil alloc is handled by init_str returning nil
        // which is safe to message (cocoa/objc handles nil messaging gracefully).
        unsafe {
            let ns_text = NSString::alloc(nil).init_str(&text);
            let _: () = msg_send![Self::as_id(handle), setStringValue: ns_text];
        }
    }
}
