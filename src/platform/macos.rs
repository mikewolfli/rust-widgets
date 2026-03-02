//! Native macOS backend using Cocoa.

#[cfg(target_os = "macos")]
mod macos_impl {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    use cocoa::appkit::{
        NSApp, NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicyRegular,
        NSBackingStoreBuffered, NSBezelStyle, NSButton, NSControl, NSRunningApplication,
        NSTextField, NSView, NSWindow, NSWindowStyleMask,
    };
    use cocoa::base::{id, nil, NO, YES};
    use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};

    use crate::core::{ObjectId, PlatformFamily};

    use super::state::BackendState;
    use super::Platform;

    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    enum HandleKind {
        /// Top-level NSWindow.
        Window,
        Button,
        CheckBox,
        LineEdit,
        MenuBar,
        Menu,
        /// NSMenuItem instance that represents a selectable action.
        MenuItem,
        ToolBar,
        StatusBar,
    }

    #[derive(Clone, Copy)]
    struct CocoaHandle {
        /// Opaque native pointer cast to usize.
        ptr: usize,
    }
    // ...existing code...
}
    /// Runtime handle kind used for dispatch.
    kind: HandleKind,
}

/// macOS desktop platform adapter.
pub struct MacOSPlatform {
    /// Shared logical widget state split from native handles.
    state: BackendState<HandleKind>,
    /// Logical id -> native handle mapping.
    handles: Mutex<HashMap<ObjectId, CocoaHandle>>,
}

static MENU_EVENTS: OnceLock<Mutex<Vec<u64>>> = OnceLock::new();
static MENU_TARGET: OnceLock<usize> = OnceLock::new();

fn menu_events() -> &'static Mutex<Vec<u64>> {
    // Shared menu-trigger queue used by Cocoa selector bridge.
    MENU_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

extern "C" fn on_menu_item(_this: &Object, _cmd: Sel, sender: id) {
    // Selector callback invoked by NSMenuItem actions.
    unsafe {
        if sender == nil {
            return;
        }
        let represented: id = msg_send![sender, representedObject];
        if represented == nil {
            return;
        }
        let item_id: u64 = msg_send![represented, unsignedLongLongValue];
        if item_id != 0 {
            menu_events()
                .lock()
                .expect("menu event lock poisoned")
                .push(item_id);
        }
    }
}

fn menu_target_class() -> *const Class {
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

fn shared_menu_target() -> id {
    let ptr = *MENU_TARGET.get_or_init(|| unsafe {
        let class = menu_target_class();
        let obj: id = msg_send![class, new];
        obj as usize
    });
    ptr as id
}

const MOD_SHIFT: u64 = 1 << 17;
const MOD_CONTROL: u64 = 1 << 18;
const MOD_OPTION: u64 = 1 << 19;
const MOD_COMMAND: u64 = 1 << 20;

fn parse_shortcut(shortcut: Option<&str>) -> (String, u64) {
    // Parse textual accelerator into Cocoa key + modifier mask.
    let Some(raw) = shortcut.map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return (String::new(), 0);
    };

    let mut modifiers: u64 = 0;
    let mut key = String::new();

    for part in raw.split('+') {
        let token = part.trim().to_lowercase();
        match token.as_str() {
            "cmd" | "command" | "meta" => modifiers |= MOD_COMMAND,
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "alt" | "option" => modifiers |= MOD_OPTION,
            "shift" => modifiers |= MOD_SHIFT,
            "cmdorctrl" => modifiers |= MOD_COMMAND,
            _ if !token.is_empty() => {
                key = token;
            }
            _ => {}
        }
    }

    if !key.is_empty() && modifiers == 0 {
        modifiers = MOD_COMMAND;
    }

    (key, modifiers)
}

impl MacOSPlatform {
    /// Creates a new macOS platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            handles: Mutex::new(HashMap::new()),
        }
    }

    fn make_rect(x: i32, y: i32, width: u32, height: u32) -> NSRect {
        NSRect::new(
            NSPoint::new(x as f64, y as f64),
            NSSize::new(width as f64, height as f64),
        )
    }

    fn window_style() -> NSWindowStyleMask {
        NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSClosableWindowMask
            | NSWindowStyleMask::NSResizableWindowMask
            | NSWindowStyleMask::NSMiniaturizableWindowMask
    }

    fn get_handle(&self, widget_id: ObjectId) -> Option<CocoaHandle> {
        self.handles
            .lock()
            .expect("macos handle lock poisoned")
            .get(&widget_id)
            .copied()
    }

    fn register_handle(
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
        self.handles
            .lock()
            .expect("macos handle lock poisoned")
            .insert(id, CocoaHandle { ptr, kind });
        id
    }

    fn as_id(handle: CocoaHandle) -> id {
        handle.ptr as id
    }
}

impl Platform for MacOSPlatform {
    fn backend_name(&self) -> &'static str {
        "cocoa"
    }

    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    fn init(&self) {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let app = NSApp();
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
            let current_app = NSRunningApplication::currentApplication(nil);
            current_app.activateWithOptions_(
                NSApplicationActivationOptions::NSApplicationActivateIgnoringOtherApps,
            );
            pool.drain();
        }
    }

    fn run(&self) {
        unsafe {
            NSApp().run();
        }
    }

    fn quit(&self) {
        unsafe {
            NSApp().stop_(nil);
        }
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                Self::make_rect(x, y, width, height),
                Self::window_style(),
                NSBackingStoreBuffered,
                NO,
            );
            window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
            NSWindow::setTitle_(window, NSString::alloc(nil).init_str(title));
            window.makeKeyAndOrderFront_(nil);

            let id = self.register_handle(HandleKind::Window, title, x, y, width, height, window as usize);

            pool.drain();
            id
        }
    }

    fn create_button(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let button = NSButton::initWithFrame_(NSButton::alloc(nil), Self::make_rect(x, y, width, height));
            NSButton::setTitle_(button, NSString::alloc(nil).init_str(text));
            NSButton::setBezelStyle_(button, NSBezelStyle::NSRoundedBezelStyle);

            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(button);
                }
            }

            let id = self.register_handle(HandleKind::Button, text, x, y, width, height, button as usize);

            pool.drain();
            id
        }
    }

    fn create_checkbox(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let button = NSButton::initWithFrame_(NSButton::alloc(nil), Self::make_rect(x, y, width, height));
            NSButton::setTitle_(button, NSString::alloc(nil).init_str(text));
            let _: () = msg_send![button, setButtonType: 3usize];

            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(button);
                }
            }

            let id = self.register_handle(HandleKind::CheckBox, text, x, y, width, height, button as usize);

            pool.drain();
            id
        }
    }

    fn create_line_edit(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let field = NSTextField::initWithFrame_(NSTextField::alloc(nil), Self::make_rect(x, y, width, height));
            NSTextField::setStringValue_(field, NSString::alloc(nil).init_str(text));

            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(field);
                }
            }

            let id = self.register_handle(HandleKind::LineEdit, text, x, y, width, height, field as usize);

            pool.drain();
            id
        }
    }

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let menu_bar: id = msg_send![class!(NSMenu), alloc];
            let menu_bar: id = msg_send![menu_bar, initWithTitle: NSString::alloc(nil).init_str("MainMenu")];
            let app = NSApp();
            let _: () = msg_send![app, setMainMenu: menu_bar];

            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let _ = Self::as_id(parent_handle);
                    let _ = (x, y, width, height);
                }
            }

            let id = self.register_handle(HandleKind::MenuBar, "MenuBar", x, y, width, height, menu_bar as usize);

            pool.drain();
            id
        }
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let menu_item: id = msg_send![class!(NSMenuItem), alloc];
            let empty = NSString::alloc(nil).init_str("");
            let menu_item: id = msg_send![
                menu_item,
                initWithTitle: NSString::alloc(nil).init_str(text)
                action: nil
                keyEquivalent: empty
            ];
            let submenu: id = msg_send![class!(NSMenu), alloc];
            let submenu: id = msg_send![submenu, initWithTitle: NSString::alloc(nil).init_str(text)];
            let _: () = msg_send![menu_item, setSubmenu: submenu];

            if let Some(parent_handle) = self.get_handle(parent) {
                let native_parent = Self::as_id(parent_handle);
                match parent_handle.kind {
                    HandleKind::MenuBar => {
                        let _: () = msg_send![native_parent, addItem: menu_item];
                    }
                    HandleKind::Menu => {
                        let parent_submenu: id = msg_send![native_parent, submenu];
                        if parent_submenu != nil {
                            let _: () = msg_send![parent_submenu, addItem: menu_item];
                        }
                    }
                    HandleKind::Window => {}
                    _ => {}
                }
            }

            let _ = (x, y, width, height);

            let id = self.register_handle(HandleKind::Menu, text, x, y, width, height, menu_item as usize);

            pool.drain();
            id
        }
    }

    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let view = NSView::initWithFrame_(NSView::alloc(nil), Self::make_rect(x, y, width, height));
            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(view);
                }
            }

            let id = self.register_handle(HandleKind::ToolBar, "ToolBar", x, y, width, height, view as usize);

            pool.drain();
            id
        }
    }

    fn create_status_bar(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let field = NSTextField::initWithFrame_(NSTextField::alloc(nil), Self::make_rect(x, y, width, height));
            NSTextField::setStringValue_(field, NSString::alloc(nil).init_str(text));
            let _: () = msg_send![field, setEditable: NO];
            let _: () = msg_send![field, setBordered: NO];

            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(field);
                }
            }

            let id = self.register_handle(HandleKind::StatusBar, text, x, y, width, height, field as usize);

            pool.drain();
            id
        }
    }

    fn attach_menu_bar_to_window(&self, _window: u64, menu_bar: u64) -> bool {
        unsafe {
            let Some(handle) = self.get_handle(menu_bar) else {
                return false;
            };
            if !matches!(handle.kind, HandleKind::MenuBar) {
                return false;
            }
            let app = NSApp();
            let _: () = msg_send![app, setMainMenu: Self::as_id(handle)];
            true
        }
    }

    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        unsafe {
            let Some(parent_handle) = self.get_handle(parent_menu) else {
                return 0;
            };

            let container: id = match parent_handle.kind {
                HandleKind::MenuBar => Self::as_id(parent_handle),
                HandleKind::Menu => {
                    let submenu: id = msg_send![Self::as_id(parent_handle), submenu];
                    if submenu == nil {
                        return 0;
                    }
                    submenu
                }
                _ => return 0,
            };

            let item_id = self.state.create_widget(HandleKind::MenuItem, text, 0, 0, 0, 0);
            let (key, modifier_mask) = parse_shortcut(shortcut);
            let item: id = msg_send![class!(NSMenuItem), alloc];
            let item: id = msg_send![
                item,
                initWithTitle: NSString::alloc(nil).init_str(text)
                action: sel!(onMenuItem:)
                keyEquivalent: NSString::alloc(nil).init_str(&key)
            ];
            if !key.is_empty() && modifier_mask != 0 {
                let _: () = msg_send![item, setKeyEquivalentModifierMask: modifier_mask];
            }

            let target = shared_menu_target();
            let _: () = msg_send![item, setTarget: target];

            let token: id = msg_send![class!(NSNumber), numberWithUnsignedLongLong: item_id];
            let _: () = msg_send![item, setRepresentedObject: token];

            let _: () = msg_send![container, addItem: item];

            self.handles
                .lock()
                .expect("macos handle lock poisoned")
                .insert(item_id, CocoaHandle { ptr: item as usize, kind: HandleKind::MenuItem });

            item_id
        }
    }

    fn poll_menu_triggered(&self) -> Option<u64> {
        let mut events = menu_events().lock().expect("menu event lock poisoned");
        if events.is_empty() {
            None
        } else {
            Some(events.remove(0))
        }
    }

    fn show_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);
        unsafe {
            if let Some(handle) = self.get_handle(widget_id) {
                let native = Self::as_id(handle);
                match handle.kind {
                    HandleKind::Window => NSWindow::makeKeyAndOrderFront_(native, nil),
                    HandleKind::MenuBar => {}
                    _ => {
                        let _: () = msg_send![native, setHidden: NO];
                    }
                }
            }
        }
    }

    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
        unsafe {
            if let Some(handle) = self.get_handle(widget_id) {
                let native = Self::as_id(handle);
                match handle.kind {
                    HandleKind::Window => NSWindow::orderOut_(native, nil),
                    HandleKind::MenuBar => {}
                    _ => {
                        let _: () = msg_send![native, setHidden: YES];
                    }
                }
            }
        }
    }

    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
        unsafe {
            if let Some(handle) = self.get_handle(widget_id) {
                let native = Self::as_id(handle);
                match handle.kind {
                    HandleKind::Window => {
                        NSWindow::setFrame_display_(native, Self::make_rect(x, y, width, height), YES);
                    }
                    HandleKind::MenuBar | HandleKind::Menu | HandleKind::MenuItem => {}
                    _ => {
                        let _: () = msg_send![native, setFrame: Self::make_rect(x, y, width, height)];
                    }
                }
            }
        }
    }

    fn set_widget_text(&self, widget_id: u64, text: &str) {
        let _ = self.state.set_text(widget_id, text);
        unsafe {
            if let Some(handle) = self.get_handle(widget_id) {
                let ns_text = NSString::alloc(nil).init_str(text);
                let native = Self::as_id(handle);
                match handle.kind {
                    HandleKind::Window => NSWindow::setTitle_(native, ns_text),
                    HandleKind::LineEdit | HandleKind::StatusBar => NSTextField::setStringValue_(native, ns_text),
                    HandleKind::MenuBar | HandleKind::ToolBar => {}
                    HandleKind::Menu | HandleKind::MenuItem => {
                        let _: () = msg_send![native, setTitle: ns_text];
                    }
                    _ => NSButton::setTitle_(native, ns_text),
                }
            }
        }
    }

    fn get_widget_text(&self, widget_id: u64) -> String {
        self.state.text(widget_id)
    }

    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
        unsafe {
            if let Some(handle) = self.get_handle(widget_id) {
                match handle.kind {
                    HandleKind::Button
                    | HandleKind::CheckBox
                    | HandleKind::LineEdit
                    | HandleKind::StatusBar => {
                        NSControl::setEnabled_(Self::as_id(handle), if enabled { YES } else { NO });
                    }
                    HandleKind::Menu | HandleKind::MenuItem => {
                        let _: () = msg_send![Self::as_id(handle), setEnabled: if enabled { YES } else { NO }];
                    }
                    _ => {}
                }
            }
        }
    }

    fn is_widget_enabled(&self, widget_id: u64) -> bool {
        self.state.enabled(widget_id)
    }

    fn set_widget_visible(&self, widget_id: u64, visible: bool) {
        if visible {
            self.show_widget(widget_id);
        } else {
            self.hide_widget(widget_id);
        }
    }

    fn is_widget_visible(&self, widget_id: u64) -> bool {
        self.state.visible(widget_id)
    }

    fn set_widget_ime_enabled(&self, widget_id: u64, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }

    fn is_widget_ime_enabled(&self, widget_id: u64) -> bool {
        self.state.ime_enabled(widget_id)
    }

    fn set_widget_accessibility_name(&self, widget_id: u64, name: &str) -> bool {
        self.state.set_accessibility_name(widget_id, name)
    }

    fn get_widget_accessibility_name(&self, widget_id: u64) -> String {
        self.state.accessibility_name(widget_id)
    }

    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }

    fn begin_drag(&self, source_widget_id: u64, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }

    fn poll_drop_event(&self) -> Option<super::DropEvent> {
        self.state.pop_drop_event()
    }

    fn inject_drop_event(&self, event: super::DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert_dummy_widget(platform: &MacOSPlatform) -> u64 {
        platform
            .state
            .create_widget(HandleKind::Button, "dummy", 0, 0, 10, 10)
    }

    #[test]
    fn macos_backend_ime_and_accessibility_state_roundtrip() {
        let platform = MacOSPlatform::new();
        let widget_id = insert_dummy_widget(&platform);

        assert!(Platform::set_widget_ime_enabled(&platform, widget_id, true));
        assert!(Platform::is_widget_ime_enabled(&platform, widget_id));

        assert!(Platform::set_widget_accessibility_name(
            &platform,
            widget_id,
            "Accessible"
        ));
        assert_eq!(
            Platform::get_widget_accessibility_name(&platform, widget_id),
            "Accessible".to_string()
        );
    }

    #[test]
    fn macos_backend_clipboard_and_drag_drop_roundtrip() {
        let platform = MacOSPlatform::new();
        let widget_id = insert_dummy_widget(&platform);

        assert!(Platform::set_clipboard_text(&platform, "hello"));
        assert_eq!(Platform::get_clipboard_text(&platform), "hello".to_string());

        assert!(Platform::begin_drag(&platform, widget_id, "text/plain", b"abc"));
        let event = Platform::poll_drop_event(&platform).expect("drop event should exist");
        assert_eq!(event.source_widget_id, widget_id);
        assert_eq!(event.mime, "text/plain");
        assert_eq!(event.payload, b"abc".to_vec());

        let injected = super::super::DropEvent {
            source_widget_id: widget_id,
            target_widget_id: widget_id,
            mime: "application/octet-stream".to_string(),
            payload: vec![1, 2, 3],
        };
        assert!(Platform::inject_drop_event(&platform, injected.clone()));
        assert_eq!(Platform::poll_drop_event(&platform), Some(injected));
    }
}
