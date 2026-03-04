//! Native macOS backend using Cocoa.
#![allow(deprecated)]

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
use super::{DropEvent, Platform};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum HandleKind {
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
}

#[derive(Clone, Copy)]
struct CocoaHandle {
    /// Opaque native pointer cast to usize.
    ptr: usize,
    /// Runtime handle kind used for dispatch.
    kind: HandleKind,
}

/// macOS desktop platform adapter.
pub struct MacOSPlatform {
    /// Shared logical widget state split from native handles.
    state: BackendState<HandleKind>,
    /// Logical id -> native handle mapping.
    handles: Mutex<HashMap<ObjectId, CocoaHandle>>,
    /// Combo-box item storage per logical widget id.
    combo_box_items: Mutex<HashMap<ObjectId, Vec<String>>>,
    /// Combo-box selected index per logical widget id.
    combo_box_selection: Mutex<HashMap<ObjectId, Option<usize>>>,
    /// List-box item storage per logical widget id.
    list_box_items: Mutex<HashMap<ObjectId, Vec<String>>>,
    /// List-box selected index per logical widget id.
    list_box_selection: Mutex<HashMap<ObjectId, Option<usize>>>,
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
            combo_box_items: Mutex::new(HashMap::new()),
            combo_box_selection: Mutex::new(HashMap::new()),
            list_box_items: Mutex::new(HashMap::new()),
            list_box_selection: Mutex::new(HashMap::new()),
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

    fn add_to_parent_window(&self, parent: ObjectId, view: id) {
        if let Some(parent_handle) = self.get_handle(parent) {
            if let HandleKind::Window = parent_handle.kind {
                unsafe {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(view);
                }
            }
        }
    }

    fn sync_list_box_native(&self, list_box: ObjectId) {
        let Some(handle) = self.get_handle(list_box) else {
            return;
        };
        if !matches!(handle.kind, HandleKind::ListBox) {
            return;
        }

        let items = self
            .list_box_items
            .lock()
            .ok()
            .and_then(|m| m.get(&list_box).cloned())
            .unwrap_or_default();
        let selected = self
            .list_box_selection
            .lock()
            .ok()
            .and_then(|m| m.get(&list_box).copied().flatten());

        let text = if items.is_empty() {
            String::new()
        } else {
            items
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    if Some(idx) == selected {
                        format!("> {item}")
                    } else {
                        item.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        unsafe {
            let ns_text = NSString::alloc(nil).init_str(&text);
            let _: () = msg_send![Self::as_id(handle), setStringValue: ns_text];
        }
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
            let app = NSApplication::sharedApplication(nil);
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
            let _: () = msg_send![app, finishLaunching];
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
            let content_view = NSView::initWithFrame_(
                NSView::alloc(nil),
                Self::make_rect(0, 0, width, height),
            );
            let _: () = msg_send![window, setContentView: content_view];
            window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
            NSWindow::setTitle_(window, NSString::alloc(nil).init_str(title));
            window.makeKeyAndOrderFront_(nil);
            let _: () = msg_send![window, display];

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

    fn create_radio_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let button = NSButton::initWithFrame_(
                NSButton::alloc(nil),
                Self::make_rect(x, y, width, height),
            );
            NSButton::setTitle_(button, NSString::alloc(nil).init_str(text));
            let _: () = msg_send![button, setButtonType: 4usize];

            self.add_to_parent_window(parent, button);

            let id = self.register_handle(
                HandleKind::RadioButton,
                text,
                x,
                y,
                width,
                height,
                button as usize,
            );

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

    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let slider: id = msg_send![class!(NSSlider), alloc];
            let slider: id = msg_send![slider, initWithFrame: Self::make_rect(x, y, width, height)];

            self.add_to_parent_window(parent, slider);

            let id = self.register_handle(
                HandleKind::Slider,
                "Slider",
                x,
                y,
                width,
                height,
                slider as usize,
            );

            pool.drain();
            id
        }
    }

    fn create_progress_bar(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let progress: id = msg_send![class!(NSProgressIndicator), alloc];
            let progress: id =
                msg_send![progress, initWithFrame: Self::make_rect(x, y, width, height)];
            let _: () = msg_send![progress, setIndeterminate: NO];
            let _: () = msg_send![progress, setMinValue: 0.0f64];
            let _: () = msg_send![progress, setMaxValue: 100.0f64];
            let _: () = msg_send![progress, setDoubleValue: 0.0f64];

            self.add_to_parent_window(parent, progress);

            let id = self.register_handle(
                HandleKind::ProgressBar,
                "ProgressBar",
                x,
                y,
                width,
                height,
                progress as usize,
            );

            pool.drain();
            id
        }
    }

    fn create_label(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let field = NSTextField::initWithFrame_(NSTextField::alloc(nil), Self::make_rect(x, y, width, height));
            NSTextField::setStringValue_(field, NSString::alloc(nil).init_str(text));
            let _: () = msg_send![field, setEditable: NO];
            let _: () = msg_send![field, setSelectable: NO];
            let _: () = msg_send![field, setBordered: NO];
            let _: () = msg_send![field, setDrawsBackground: NO];

            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(field);
                }
            }

            let id = self.register_handle(HandleKind::Label, text, x, y, width, height, field as usize);

            pool.drain();
            id
        }
    }

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let menu_bar: id = msg_send![class!(NSMenu), alloc];
            let menu_bar: id = msg_send![menu_bar, initWithTitle: NSString::alloc(nil).init_str("MainMenu")];
            let _: () = msg_send![menu_bar, setAutoenablesItems: NO];

            let app_menu_item: id = msg_send![class!(NSMenuItem), alloc];
            let app_menu_item: id = msg_send![
                app_menu_item,
                initWithTitle: NSString::alloc(nil).init_str("")
                action: nil
                keyEquivalent: NSString::alloc(nil).init_str("")
            ];
            let _: () = msg_send![menu_bar, addItem: app_menu_item];

            let app_menu: id = msg_send![class!(NSMenu), alloc];
            let app_menu: id = msg_send![app_menu, initWithTitle: NSString::alloc(nil).init_str("Application")];
            let _: () = msg_send![app_menu, setAutoenablesItems: NO];
            let _: () = msg_send![menu_bar, setSubmenu: app_menu forItem: app_menu_item];

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
            let _: () = msg_send![submenu, setAutoenablesItems: NO];
            let _: () = msg_send![menu_item, setSubmenu: submenu];

            if let Some(parent_handle) = self.get_handle(parent) {
                let native_parent = Self::as_id(parent_handle);
                match parent_handle.kind {
                    HandleKind::MenuBar => {
                        let _: () = msg_send![native_parent, addItem: menu_item];
                        let _: () = msg_send![native_parent, setSubmenu: submenu forItem: menu_item];
                    }
                    HandleKind::Menu => {
                        let parent_submenu: id = msg_send![native_parent, submenu];
                        if parent_submenu != nil {
                            let _: () = msg_send![parent_submenu, addItem: menu_item];
                            let _: () = msg_send![parent_submenu, setSubmenu: submenu forItem: menu_item];
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

    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let combo: id = msg_send![class!(NSPopUpButton), alloc];
            let combo: id = msg_send![combo, initWithFrame: Self::make_rect(x, y, width, height) pullsDown: NO];

            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(combo);
                }
            }

            let id = self.register_handle(
                HandleKind::ComboBox,
                "ComboBox",
                x,
                y,
                width,
                height,
                combo as usize,
            );

            self.combo_box_items
                .lock()
                .expect("macos combo item lock poisoned")
                .insert(id, Vec::new());
            self.combo_box_selection
                .lock()
                .expect("macos combo selection lock poisoned")
                .insert(id, None);

            pool.drain();
            id
        }
    }

    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let field = NSTextField::initWithFrame_(
                NSTextField::alloc(nil),
                Self::make_rect(x, y, width, height),
            );
            NSTextField::setStringValue_(field, NSString::alloc(nil).init_str(""));
            let _: () = msg_send![field, setEditable: NO];
            let _: () = msg_send![field, setSelectable: NO];
            let _: () = msg_send![field, setBordered: YES];
            let _: () = msg_send![field, setBezeled: YES];

            self.add_to_parent_window(parent, field);

            let id = self.register_handle(
                HandleKind::ListBox,
                "ListBox",
                x,
                y,
                width,
                height,
                field as usize,
            );

            self.list_box_items
                .lock()
                .expect("macos list item lock poisoned")
                .insert(id, Vec::new());
            self.list_box_selection
                .lock()
                .expect("macos list selection lock poisoned")
                .insert(id, None);

            pool.drain();
            id
        }
    }

    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        let Some(handle) = self.get_handle(list_box) else {
            return false;
        };
        if !matches!(handle.kind, HandleKind::ListBox) {
            return false;
        }

        if let Ok(mut items) = self.list_box_items.lock() {
            items.entry(list_box).or_default().push(text.to_string());
        } else {
            return false;
        }
        self.sync_list_box_native(list_box);
        true
    }

    fn list_box_remove_item(&self, list_box: u64, index: usize) -> bool {
        let Some(handle) = self.get_handle(list_box) else {
            return false;
        };
        if !matches!(handle.kind, HandleKind::ListBox) {
            return false;
        }

        if let Ok(mut items) = self.list_box_items.lock() {
            let Some(list) = items.get_mut(&list_box) else {
                return false;
            };
            if index >= list.len() {
                return false;
            }
            list.remove(index);
            if let Ok(mut selection) = self.list_box_selection.lock() {
                match selection.get(&list_box).copied().flatten() {
                    Some(sel) if sel == index => {
                        selection.insert(list_box, None);
                    }
                    Some(sel) if sel > index => {
                        selection.insert(list_box, Some(sel - 1));
                    }
                    _ => {}
                }
            }
        } else {
            return false;
        }

        self.sync_list_box_native(list_box);
        true
    }

    fn list_box_clear_items(&self, list_box: u64) -> bool {
        let Some(handle) = self.get_handle(list_box) else {
            return false;
        };
        if !matches!(handle.kind, HandleKind::ListBox) {
            return false;
        }

        if let Ok(mut items) = self.list_box_items.lock() {
            items.insert(list_box, Vec::new());
        } else {
            return false;
        }
        if let Ok(mut selection) = self.list_box_selection.lock() {
            selection.insert(list_box, None);
        }
        self.sync_list_box_native(list_box);
        true
    }

    fn list_box_set_current_index(&self, list_box: u64, index: usize) -> bool {
        let Some(handle) = self.get_handle(list_box) else {
            return false;
        };
        if !matches!(handle.kind, HandleKind::ListBox) {
            return false;
        }

        let count = self.list_box_item_count(list_box);
        if index >= count {
            return false;
        }

        if let Ok(mut selection) = self.list_box_selection.lock() {
            selection.insert(list_box, Some(index));
        } else {
            return false;
        }
        self.sync_list_box_native(list_box);
        true
    }

    fn list_box_current_index(&self, list_box: u64) -> Option<usize> {
        self.list_box_selection
            .lock()
            .ok()
            .and_then(|selection| selection.get(&list_box).copied().flatten())
    }

    fn list_box_item_count(&self, list_box: u64) -> usize {
        self.list_box_items
            .lock()
            .ok()
            .and_then(|items| items.get(&list_box).map(|v| v.len()))
            .unwrap_or(0)
    }

    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        self.list_box_items
            .lock()
            .ok()
            .and_then(|items| items.get(&list_box).and_then(|v| v.get(index).cloned()))
    }

    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let view = NSView::initWithFrame_(NSView::alloc(nil), Self::make_rect(x, y, width, height));
            self.add_to_parent_window(parent, view);

            let id = self.register_handle(
                HandleKind::Panel,
                "Panel",
                x,
                y,
                width,
                height,
                view as usize,
            );

            pool.drain();
            id
        }
    }

    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        let Some(handle) = self.get_handle(combo_box) else {
            return false;
        };
        if !matches!(handle.kind, HandleKind::ComboBox) {
            return false;
        }

        unsafe {
            let title = NSString::alloc(nil).init_str(text);
            let _: () = msg_send![Self::as_id(handle), addItemWithTitle: title];
        }

        if let Ok(mut items) = self.combo_box_items.lock() {
            items.entry(combo_box).or_default().push(text.to_string());
            true
        } else {
            false
        }
    }

    fn combo_box_clear_items(&self, combo_box: u64) -> bool {
        let Some(handle) = self.get_handle(combo_box) else {
            return false;
        };
        if !matches!(handle.kind, HandleKind::ComboBox) {
            return false;
        }

        unsafe {
            let _: () = msg_send![Self::as_id(handle), removeAllItems];
        }

        if let Ok(mut items) = self.combo_box_items.lock() {
            items.insert(combo_box, Vec::new());
        } else {
            return false;
        }
        if let Ok(mut selection) = self.combo_box_selection.lock() {
            selection.insert(combo_box, None);
            true
        } else {
            false
        }
    }

    fn combo_box_set_current_index(&self, combo_box: u64, index: usize) -> bool {
        let Some(handle) = self.get_handle(combo_box) else {
            return false;
        };
        if !matches!(handle.kind, HandleKind::ComboBox) {
            return false;
        }

        let count = self.combo_box_item_count(combo_box);
        if index >= count {
            return false;
        }

        unsafe {
            let _: () = msg_send![Self::as_id(handle), selectItemAtIndex: index as isize];
        }

        if let Ok(mut selection) = self.combo_box_selection.lock() {
            selection.insert(combo_box, Some(index));
            true
        } else {
            false
        }
    }

    fn combo_box_current_index(&self, combo_box: u64) -> Option<usize> {
        self.combo_box_selection
            .lock()
            .ok()
            .and_then(|selection| selection.get(&combo_box).copied().flatten())
    }

    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        self.combo_box_items
            .lock()
            .ok()
            .and_then(|items| items.get(&combo_box).map(|v| v.len()))
            .unwrap_or(0)
    }

    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        self.combo_box_items
            .lock()
            .ok()
            .and_then(|items| items.get(&combo_box).and_then(|v| v.get(index).cloned()))
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
                    let _: () = msg_send![submenu, setAutoenablesItems: NO];
                    submenu
                }
                _ => return 0,
            };

            let _: () = msg_send![container, setAutoenablesItems: NO];

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
                    HandleKind::LineEdit | HandleKind::Label | HandleKind::StatusBar => NSTextField::setStringValue_(native, ns_text),
                    HandleKind::ComboBox => {}
                    HandleKind::ListBox => {
                        let _: () = msg_send![native, setStringValue: ns_text];
                    }
                    HandleKind::Slider | HandleKind::ProgressBar => {
                        if let Ok(value) = text.parse::<f64>() {
                            let _: () = msg_send![native, setDoubleValue: value];
                        }
                    }
                    HandleKind::MenuBar | HandleKind::ToolBar => {}
                    HandleKind::Panel => {}
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
                    | HandleKind::RadioButton
                    | HandleKind::Label
                    | HandleKind::LineEdit
                    | HandleKind::Slider
                    | HandleKind::ProgressBar
                    | HandleKind::ComboBox
                    | HandleKind::ListBox
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

    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }

    fn inject_drop_event(&self, event: DropEvent) -> bool {
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
        let platform = new();
        let widget_id = insert_dummy_widget(&platform);

        assert!(set_widget_ime_enabled(&platform, widget_id, true));
        assert!(is_widget_ime_enabled(&platform, widget_id));

        assert!(set_widget_accessibility_name(
            &platform,
            widget_id,
            "Accessible"
        ));
        assert_eq!(
            get_widget_accessibility_name(&platform, widget_id),
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

        let injected = super::DropEvent {
            source_widget_id: widget_id,
            target_widget_id: widget_id,
            mime: "application/octet-stream".to_string(),
            payload: vec![1, 2, 3],
        };
        assert!(Platform::inject_drop_event(&platform, injected.clone()));
        assert_eq!(Platform::poll_drop_event(&platform), Some(injected));
    }

}
