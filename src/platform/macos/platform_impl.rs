//! `impl Platform for MacOSPlatform` — the main trait implementation.

#![allow(deprecated)] // Cocoa 0.24 fallback; remove when objc2 backend fully replaces cocoa

use crate::core::{ObjectId, PlatformFamily};
use crate::platform::accessibility::AccessibilityBridge;
use crate::platform::clipboard::RichClipboardBackend;
use crate::platform::ime::ImeBridge;
use crate::platform::macos::types::*;
use crate::platform::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicyRegular,
    NSBackingStoreBuffered, NSBezelStyle, NSButton, NSControl, NSRunningApplication, NSTextField,
    NSView, NSWindow,
};
use cocoa::base::{id, nil, BOOL, NO, YES};
use cocoa::foundation::{NSArray, NSAutoreleasePool, NSData, NSPoint, NSString};
use objc::runtime::Sel;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CStr;
use std::os::raw::c_char;

impl Platform for MacOSPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "cocoa"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }
    fn init(&self) {
        // SAFETY: NSAutoreleasePool::new(nil) is safe per Apple's documentation
        // (nil argument is allowed). NSApplication sharedApplication and messaging
        // are called on the main thread, which is required by Cocoa. All Objective-C
        // message sends use valid selectors from the cocoa/objc crates.
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
        // SAFETY: NSApp() returns the shared application instance initialized in init().
        // run() must be called on the main thread, which is guaranteed by the platform
        // contract (init is called before run on the same thread).
        unsafe {
            NSApp().run();
        }
    }
    fn quit(&self) {
        // SAFETY: NSApp().stop_(nil) is safe to call on the main thread after the
        // application has been initialized. The nil argument tells the app to stop
        // without a specific sender.
        unsafe {
            NSApp().stop_(nil);
        }
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // SAFETY: Cocoa APIs require the main thread, guaranteed by the platform contract.
        // NSAutoreleasePool::new(nil) is safe with nil argument. All Objective-C messages
        // use valid selectors from the cocoa crate. Self::register_handle() stores the
        // raw pointer cast as usize without aliasing issues. Nil returns from alloc are
        // checked and logged before proceeding.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            // Check for nil after NSWindow::alloc — Cocoa returns nil on allocation failure.
            let raw_window = NSWindow::alloc(nil);
            if raw_window == nil {
                log::error!("[macos] create_window: NSWindow::alloc returned nil (out of memory?)");
                pool.drain();
                return 0;
            }
            let window = raw_window.initWithContentRect_styleMask_backing_defer_(
                Self::make_rect(x, y, width, height),
                Self::window_style(),
                NSBackingStoreBuffered,
                NO,
            );

            // Check for nil after NSView::alloc
            let raw_content = NSView::alloc(nil);
            let content_view = if raw_content == nil {
                log::error!("[macos] create_window: NSView::alloc returned nil (out of memory?)");
                pool.drain();
                return 0;
            } else {
                NSView::initWithFrame_(raw_content, Self::make_rect(0, 0, width, height))
            };

            // Check for nil after NSString::alloc for the title
            let raw_title = NSString::alloc(nil);
            if raw_title == nil {
                log::error!("[macos] create_window: NSString::alloc returned nil (out of memory?)");
                pool.drain();
                return 0;
            }

            let _: () = msg_send![window, setContentView: content_view];
            window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
            NSWindow::setTitle_(window, raw_title.init_str(title));
            window.makeKeyAndOrderFront_(nil);
            let _: () = msg_send![window, display];
            let id = self.register_handle(
                HandleKind::Window,
                title,
                x,
                y,
                width,
                height,
                window as usize,
            );
            pool.drain();
            id
        }
    }
    fn create_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        log::info!(
            "[rust_widgets] MacOSPlatform::create_button called: parent={}, text='{}'",
            parent,
            text
        );
        // SAFETY: All Objective-C messages in this block use valid selectors from the
        // cocoa/objc crates, called on the main thread. NSButton::alloc(nil) and
        // NSString::alloc(nil) are checked for nil returns before use. The parent
        // handle is validated against the handle registry before use.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            // Check for nil after NSButton::alloc
            let raw_button = NSButton::alloc(nil);
            if raw_button == nil {
                log::error!("[macos] create_button: NSButton::alloc returned nil (out of memory?)");
                pool.drain();
                return 0;
            }
            let button = NSButton::initWithFrame_(raw_button, Self::make_rect(x, y, width, height));
            log::info!("[rust_widgets] MacOSPlatform::create_button: button created {:?}", button);

            // Check for nil after NSString::alloc for the button title
            let raw_title = NSString::alloc(nil);
            if raw_title == nil {
                log::error!("[macos] create_button: NSString::alloc returned nil (out of memory?)");
                pool.drain();
                return 0;
            }
            NSButton::setTitle_(button, raw_title.init_str(text));
            NSButton::setBezelStyle_(button, NSBezelStyle::NSRoundedBezelStyle);
            // Set button type to momentary push button
            let _: () = msg_send![button, setButtonType: 0u64]; // NSMomentaryPushInButton
                                                                // Enable the button
            let _: () = msg_send![button, setEnabled: YES];
            // Set button to send action on mouse up and mouse down
            let _: () = msg_send![button, sendActionOn: 2u64]; // NSLeftMouseDownMask
            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(button);
                }
            }
            let id = self.register_handle(
                HandleKind::Button,
                text,
                x,
                y,
                width,
                height,
                button as usize,
            );
            log::info!("[rust_widgets] create_button: created button with id {}", id);
            // Set up button click handler using NSButton methods
            let target = shared_button_target();
            log::error!("[rust_widgets] create_button: setting target {:?}", target);
            NSButton::setTarget_(button, target);
            log::error!("[rust_widgets] create_button: setting action");
            let action_sel = sel!(onButtonClicked:);
            log::debug!("[rust_widgets] create_button: action selector = {:?}", action_sel);
            NSButton::setAction_(button, action_sel);
            // Test the button action
            let _: () = msg_send![button, performClick: nil];
            log::error!("[rust_widgets] create_button: performed test click");
            // Verify the target and action are set correctly
            let current_target: id = msg_send![button, target];
            let current_action: Sel = msg_send![button, action];
            log::debug!(
                "[rust_widgets] create_button: current target = {:?}, current action = {:?}",
                current_target,
                current_action
            );
            // Create NSNumber to store widget id - use numberWithUnsignedLongLong
            log::error!("[rust_widgets] create_button: creating token for id {}", id);
            let token: id = msg_send![class!(NSNumber), numberWithUnsignedLongLong: id];
            // Retain the token to prevent it from being released
            let _: () = msg_send![token, retain];
            let _: () = msg_send![button, setRepresentedObject: token];
            log::error!("[rust_widgets] create_button: done");
            pool.drain();
            id
        }
    }
    fn create_checkbox(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // SAFETY: All Objective-C messages use valid selectors from the cocoa crate.
        // NSButton::alloc(nil) returns a valid instance. The parent handle is validated
        // before dereference. The token NSNumber is retained to prevent premature release.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let button = NSButton::initWithFrame_(
                NSButton::alloc(nil),
                Self::make_rect(x, y, width, height),
            );
            NSButton::setTitle_(button, NSString::alloc(nil).init_str(text));
            let _: () = msg_send![button, setButtonType: 3usize];
            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(button);
                }
            }
            let id = self.register_handle(
                HandleKind::CheckBox,
                text,
                x,
                y,
                width,
                height,
                button as usize,
            );
            // Set up checkbox click handler
            let target = shared_button_target();
            NSButton::setTarget_(button, target);
            NSButton::setAction_(button, sel!(onButtonClicked:));
            let token: id = msg_send![class!(NSNumber), numberWithUnsignedLongLong: id];
            let _: () = msg_send![token, retain];
            let _: () = msg_send![button, setRepresentedObject: token];
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
        // SAFETY: Same pattern as create_checkbox - valid ObjC selectors,
        // validated parent handle, retained token for widget ID mapping.
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
            // Set up radio button click handler
            let target = shared_button_target();
            NSButton::setTarget_(button, target);
            NSButton::setAction_(button, sel!(onButtonClicked:));
            let token: id = msg_send![class!(NSNumber), numberWithUnsignedLongLong: id];
            let _: () = msg_send![token, retain];
            let _: () = msg_send![button, setRepresentedObject: token];
            pool.drain();
            id
        }
    }
    fn create_line_edit(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // SAFETY: ObjC messages for NSScrollView and NSTextView use valid class
        // names and selectors from the Cocoa runtime. alloc/init pairs are balanced.
        // The parent handle is validated before adding the scroll view as subview.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            // Create scroll view first
            let scroll_view: id = msg_send![class!(NSScrollView), alloc];
            let scroll_view: id =
                msg_send![scroll_view, initWithFrame: Self::make_rect(x, y, width, height)];
            // Create text view with frame
            let text_view: id = msg_send![class!(NSTextView), alloc];
            let text_view: id =
                msg_send![text_view, initWithFrame: Self::make_rect(0, 0, width, height)];
            // Configure text view
            let _: () = msg_send![text_view, setEditable: NO];
            let _: () = msg_send![text_view, setSelectable: YES];
            let _: () = msg_send![text_view, setAutoresizingMask: 18u64]; // NSViewWidthSizable | NSViewHeightSizable
                                                                          // Set font
            let font: id = msg_send![class!(NSFont), systemFontOfSize: 10.0f64];
            let _: () = msg_send![text_view, setFont: font];
            // Set initial text
            let ns_text = NSString::alloc(nil).init_str(text);
            let _: () = msg_send![text_view, setString: ns_text];
            // Configure scroll view
            let _: () = msg_send![scroll_view, setDocumentView: text_view];
            let _: () = msg_send![scroll_view, setHasVerticalScroller: YES];
            let _: () = msg_send![scroll_view, setAutoresizingMask: 18u64];
            let _: () = msg_send![scroll_view, setBorderType: 2u64]; // NSBezelBorder
            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(scroll_view);
                }
            }
            let id = self.register_handle(
                HandleKind::LineEdit,
                text,
                x,
                y,
                width,
                height,
                text_view as usize,
            );
            pool.drain();
            id
        }
    }
    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // SAFETY: NSSlider class exists in the Cocoa runtime. alloc/init/frame
        // messages use valid selectors. Parent handle is validated by add_to_parent_window.
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
    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // SAFETY: NSProgressIndicator is a standard Cocoa class. All selectors used
        // (setIndeterminate:, setMinValue:, etc.) are valid. Parent validated by
        // add_to_parent_window.
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
    fn create_label(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // SAFETY: NSTextField alloc/init and configuration messages use valid
        // selectors. Parent handle is validated before adding subview.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let field = NSTextField::initWithFrame_(
                NSTextField::alloc(nil),
                Self::make_rect(x, y, width, height),
            );
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
            let id =
                self.register_handle(HandleKind::Label, text, x, y, width, height, field as usize);
            pool.drain();
            id
        }
    }
    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // SAFETY: NSMenu and NSMenuItem are standard Cocoa classes. App's main menu
        // is set via NSApp(), which is initialized. alloc/init pairs are balanced.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let menu_bar: id = msg_send![class!(NSMenu), alloc];
            let menu_bar: id =
                msg_send![menu_bar, initWithTitle: NSString::alloc(nil).init_str("MainMenu")];
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
            let app_menu: id =
                msg_send![app_menu, initWithTitle: NSString::alloc(nil).init_str("Application")];
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
            let id = self.register_handle(
                HandleKind::MenuBar,
                "MenuBar",
                x,
                y,
                width,
                height,
                menu_bar as usize,
            );
            pool.drain();
            id
        }
    }
    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // SAFETY: NSMenuItem/NSMenu alloc/init messages use valid selectors.
        // Parent handle is matched against known HandleKind variants, ensuring
        // only valid native pointers are dereferenced.
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
            let submenu: id =
                msg_send![submenu, initWithTitle: NSString::alloc(nil).init_str(text)];
            let _: () = msg_send![submenu, setAutoenablesItems: NO];
            let _: () = msg_send![menu_item, setSubmenu: submenu];
            if let Some(parent_handle) = self.get_handle(parent) {
                let native_parent = Self::as_id(parent_handle);
                match parent_handle.kind {
                    HandleKind::MenuBar => {
                        let _: () = msg_send![native_parent, addItem: menu_item];
                        let _: () =
                            msg_send![native_parent, setSubmenu: submenu forItem: menu_item];
                    }
                    HandleKind::Menu => {
                        let parent_submenu: id = msg_send![native_parent, submenu];
                        if parent_submenu != nil {
                            let _: () = msg_send![parent_submenu, addItem: menu_item];
                            let _: () =
                                msg_send![parent_submenu, setSubmenu: submenu forItem: menu_item];
                        }
                    }
                    HandleKind::Window => {}
                    // Other handle types need no special handling
                    _ => {}
                }
            }
            let _ = (x, y, width, height);
            let id = self.register_handle(
                HandleKind::Menu,
                text,
                x,
                y,
                width,
                height,
                menu_item as usize,
            );
            pool.drain();
            id
        }
    }
    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // SAFETY: NSView alloc/init/frame messages use valid selectors.
        // Parent handle is validated by kind matching before adding subview.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let view =
                NSView::initWithFrame_(NSView::alloc(nil), Self::make_rect(x, y, width, height));
            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(view);
                }
            }
            let id = self.register_handle(
                HandleKind::ToolBar,
                "ToolBar",
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
    fn create_status_bar(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // SAFETY: NSTextField messages use valid selectors (setEditable:, setBordered:, etc.).
        // Parent handle is validated before adding to window content view.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let field = NSTextField::initWithFrame_(
                NSTextField::alloc(nil),
                Self::make_rect(x, y, width, height),
            );
            NSTextField::setStringValue_(field, NSString::alloc(nil).init_str(text));
            let _: () = msg_send![field, setEditable: NO];
            let _: () = msg_send![field, setBordered: NO];
            if let Some(parent_handle) = self.get_handle(parent) {
                if let HandleKind::Window = parent_handle.kind {
                    let content_view = NSWindow::contentView(Self::as_id(parent_handle));
                    content_view.addSubview_(field);
                }
            }
            let id = self.register_handle(
                HandleKind::StatusBar,
                text,
                x,
                y,
                width,
                height,
                field as usize,
            );
            pool.drain();
            id
        }
    }
    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // SAFETY: NSPopUpButton is a standard Cocoa class. alloc/init messages use
        // valid selectors. Parent handle is validated before adding subview.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let combo: id = msg_send![class!(NSPopUpButton), alloc];
            let combo: id =
                msg_send![combo, initWithFrame: Self::make_rect(x, y, width, height) pullsDown: NO];
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
        // SAFETY: NSTextField configuration messages use valid selectors.
        // Parent handle is validated before adding subview.
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
                    // No adjustment needed for this case
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
        match self.list_box_selection.lock() {
            Ok(selection) => selection.get(&list_box).copied().flatten(),
            Err(_) => {
                log::error!(
                    "[rust_widgets] list_box_current_index: list_box_selection mutex poisoned"
                );
                None
            }
        }
    }
    fn list_box_item_count(&self, list_box: u64) -> usize {
        match self.list_box_items.lock() {
            Ok(items) => items.get(&list_box).map(|v| v.len()).unwrap_or(0),
            Err(_) => {
                log::error!("[rust_widgets] list_box_item_count: list_box_items mutex poisoned");
                0
            }
        }
    }
    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        match self.list_box_items.lock() {
            Ok(items) => items.get(&list_box).and_then(|v| v.get(index).cloned()),
            Err(_) => {
                log::error!("[rust_widgets] list_box_item_text: list_box_items mutex poisoned");
                None
            }
        }
    }
    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // SAFETY: NSView alloc/init/frame messages use valid selectors.
        // Parent handle is validated by add_to_parent_window.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let view =
                NSView::initWithFrame_(NSView::alloc(nil), Self::make_rect(x, y, width, height));
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
        // SAFETY: handle has been validated by the matches! check above.
        // Self::as_id(handle) converts the stored usize back to a valid ObjC id.
        // addItemWithTitle: is a valid selector on NSPopUpButton.
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
        // SAFETY: handle validated by kind match; Self::as_id converts stored
        // usize to valid ObjC id. removeAllItems is a valid selector on NSPopUpButton.
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
        // SAFETY: handle validated by kind match. index is checked against item count.
        // selectItemAtIndex: is a valid selector on NSPopUpButton.
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
        match self.combo_box_selection.lock() {
            Ok(selection) => selection.get(&combo_box).copied().flatten(),
            Err(_) => {
                log::error!(
                    "[rust_widgets] combo_box_current_index: combo_box_selection mutex poisoned"
                );
                None
            }
        }
    }
    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        match self.combo_box_items.lock() {
            Ok(items) => items.get(&combo_box).map(|v| v.len()).unwrap_or(0),
            Err(_) => {
                log::error!("[rust_widgets] combo_box_item_count: combo_box_items mutex poisoned");
                0
            }
        }
    }
    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        match self.combo_box_items.lock() {
            Ok(items) => items.get(&combo_box).and_then(|v| v.get(index).cloned()),
            Err(_) => {
                log::error!("[rust_widgets] combo_box_item_text: combo_box_items mutex poisoned");
                None
            }
        }
    }
    fn attach_menu_bar_to_window(&self, _window: u64, menu_bar: u64) -> bool {
        // SAFETY: handle validated by kind match. NSApp() is initialized.
        // setMainMenu: is a valid selector on NSApplication.
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
        // SAFETY: Parent handle validated by kind match. NSMenuItem/NSMenu alloc/init
        // and configuration messages use valid selectors. Token NSNumber is retained.
        // sel!(onMenuItem:) is registered by the ObjC runtime initialization.
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
        // SAFETY: handle validated by get_handle; kind is matched to choose the
        // correct ObjC message. makeKeyAndOrderFront: and setHidden: are valid
        // selectors on their respective Cocoa classes.
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
        // SAFETY: handle validated by get_handle; kind is matched to choose the
        // correct ObjC message. orderOut: and setHidden: are valid selectors.
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
        // SAFETY: handle validated by get_handle; kind is matched to choose the
        // correct ObjC message (setFrame:display: for windows, setFrame: for views).
        unsafe {
            if let Some(handle) = self.get_handle(widget_id) {
                let native = Self::as_id(handle);
                match handle.kind {
                    HandleKind::Window => {
                        NSWindow::setFrame_display_(
                            native,
                            Self::make_rect(x, y, width, height),
                            YES,
                        );
                    }
                    HandleKind::MenuBar | HandleKind::Menu | HandleKind::MenuItem => {}
                    _ => {
                        let _: () =
                            msg_send![native, setFrame: Self::make_rect(x, y, width, height)];
                    }
                }
            }
        }
    }
    fn set_widget_text(&self, widget_id: u64, text: &str) {
        let _ = self.state.set_text(widget_id, text);
        // SAFETY: handle validated by get_handle; kind is matched to dispatch to
        // the correct ObjC selector (setTitle:, setStringValue:, setDoubleValue:).
        // NSString::alloc(nil).init_str(text) produces a valid NSString that is
        // autoreleased. Main thread dispatch uses performSelectorOnMainThread:withObject:
        // which is safe when the NSString is retained before dispatch.
        unsafe {
            if let Some(handle) = self.get_handle(widget_id) {
                let ns_text = NSString::alloc(nil).init_str(text);
                let native = Self::as_id(handle);
                // Check if we're on the main thread
                let is_main_thread: bool = msg_send![class!(NSThread), isMainThread];
                if !is_main_thread {
                    // For non-main thread, we need to dispatch to main thread
                    // Use performSelectorOnMainThread with the control itself
                    let _: () = msg_send![ns_text, retain];
                    log::error!("[rust_widgets] set_widget_text: dispatching to main thread, native={:?}, ns_text={:?}", native, ns_text);
                    // For NSTextField, use setStringValue: selector
                    let selector = sel!(setStringValue:);
                    let result: bool = msg_send![native, respondsToSelector:selector];
                    log::debug!(
                        "[rust_widgets] set_widget_text: native responds to setStringValue: ? {}",
                        result
                    );
                    if result {
                        let _: () = msg_send![native, performSelectorOnMainThread:selector withObject:ns_text waitUntilDone:YES];
                        log::error!("[rust_widgets] set_widget_text: dispatched to main thread with setStringValue:");
                    } else {
                        // Fallback: try setString: selector (NSTextView)
                        let selector2 = sel!(setString:);
                        let result2: bool = msg_send![native, respondsToSelector:selector2];
                        log::debug!(
                            "[rust_widgets] set_widget_text: native responds to setString: ? {}",
                            result2
                        );
                        if result2 {
                            let _: () = msg_send![native, performSelectorOnMainThread:selector2 withObject:ns_text waitUntilDone:YES];
                            log::error!("[rust_widgets] set_widget_text: dispatched to main thread with setString:");
                        }
                    }
                } else {
                    match handle.kind {
                        HandleKind::Window => NSWindow::setTitle_(native, ns_text),
                        HandleKind::LineEdit => {
                            // NSTextField uses setStringValue: selector
                            let _: () = msg_send![native, setStringValue: ns_text];
                        }
                        HandleKind::Label | HandleKind::StatusBar => {
                            NSTextField::setStringValue_(native, ns_text)
                        }
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
    }
    fn get_widget_text(&self, widget_id: u64) -> String {
        self.state.text(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
        // SAFETY: handle validated by get_handle; kind is matched to choose the
        // appropriate Cocoa control. setEnabled: is a valid selector on NSControl
        // and NSMenuItem. Self::as_id(handle) restores the valid ObjC pointer.
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
                    // Other handle types need no special handling
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
        // If no native handle, fall back to state immediately.
        if self.get_handle(widget_id).is_none() {
            return self.state.set_accessibility_name(widget_id, name);
        }
        // Try native ObjC setAccessibilityLabel: on the NSView/NSControl
        let result = std::panic::catch_unwind(|| unsafe {
            let handle = self.get_handle(widget_id).unwrap();
            let ns_str = NSString::alloc(nil).init_str(name);
            let _: () = msg_send![Self::as_id(handle), setAccessibilityLabel: ns_str];
            true
        });
        // Fall back to state on ObjC failure
        result.unwrap_or_else(|_| self.state.set_accessibility_name(widget_id, name))
    }
    fn get_widget_accessibility_name(&self, widget_id: u64) -> String {
        // If no native handle, fall back to state immediately.
        if self.get_handle(widget_id).is_none() {
            return self.state.accessibility_name(widget_id);
        }
        // Try native ObjC accessibilityLabel on the NSView/NSControl
        let result = std::panic::catch_unwind(|| unsafe {
            let handle = self.get_handle(widget_id).unwrap();
            let label: id = msg_send![Self::as_id(handle), accessibilityLabel];
            if label != nil {
                let c_str: *const c_char = msg_send![label, UTF8String];
                if !c_str.is_null() {
                    return Some(CStr::from_ptr(c_str).to_string_lossy().into_owned());
                }
            }
            None
        });
        // Fall back to state on ObjC failure
        result.unwrap_or(None).unwrap_or_else(|| self.state.accessibility_name(widget_id))
    }
    fn set_clipboard_text(&self, text: &str) -> bool {
        // Try real NSPasteboard integration first
        let result = std::panic::catch_unwind(|| unsafe {
            let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
            if pb == nil {
                return false;
            }
            let _: () = msg_send![pb, clearContents];
            let ns_str = NSString::alloc(nil).init_str(text);
            let type_str = NSString::alloc(nil).init_str("public.utf8-plain-text");
            let success: BOOL = msg_send![pb, setString:ns_str forType:type_str];
            success != NO
        });
        // Fall back to state on ObjC failure (including panics)
        result.unwrap_or_else(|_| self.state.set_clipboard_text(text))
    }
    fn get_clipboard_text(&self) -> String {
        // Try real NSPasteboard integration first
        let result = std::panic::catch_unwind(|| unsafe {
            let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
            if pb == nil {
                return None;
            }
            let type_str = NSString::alloc(nil).init_str("public.utf8-plain-text");
            let text_obj: id = msg_send![pb, stringForType:type_str];
            if text_obj == nil {
                return None;
            }
            let c_str: *const c_char = msg_send![text_obj, UTF8String];
            if c_str.is_null() {
                return None;
            }
            Some(CStr::from_ptr(c_str).to_string_lossy().into_owned())
        });
        // Fall back to state on ObjC failure
        result.unwrap_or(None).unwrap_or_else(|| self.state.clipboard_text())
    }
    fn begin_drag(&self, source_widget_id: u64, mime: &str, payload: &[u8]) -> bool {
        // If no native handle exists, fall back to state immediately.
        if self.get_handle(source_widget_id).is_none() {
            return self.state.begin_drag(source_widget_id, mime, payload);
        }
        // Try real NSPasteboardItem drag session first
        let result = std::panic::catch_unwind(|| unsafe {
            let handle = self.get_handle(source_widget_id).unwrap();
            let view = Self::as_id(handle);
            let item: id = msg_send![class!(NSPasteboardItem), alloc];
            let item: id = msg_send![item, init];
            if item == nil {
                return false;
            }
            let uti = NSString::alloc(nil).init_str(mime);
            let ns_data = NSData::dataWithBytes_length_(
                nil,
                payload.as_ptr() as *const std::ffi::c_void,
                payload.len() as u64,
            );
            let set_ok: BOOL = msg_send![item, setData:ns_data forType:uti];
            if set_ok == NO {
                return false;
            }
            // Create NSDraggingItem with NSPasteboardItem as pasteboard writer
            let drag_item: id = msg_send![class!(NSDraggingItem), alloc];
            let drag_item: id = msg_send![drag_item, initWithPasteboardWriter:item];
            if drag_item == nil {
                return false;
            }
            // Build NSArray of dragging items
            let items_array = NSArray::arrayWithObjects(nil, &[drag_item]);
            let current_event: id = msg_send![class!(NSEvent), currentEvent];
            // Start drag session
            let _: id = msg_send![view, beginDraggingSessionWithItems:items_array event:current_event source:view];
            true
        });
        // Fall back to state on ObjC failure
        result.unwrap_or_else(|_| self.state.begin_drag(source_widget_id, mime, payload))
    }
    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }
    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        self.state.inject_menu_trigger(menu_item_id)
    }
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.state.pop_widget_trigger()
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        // First check native widget events from Cocoa
        if let Ok(mut events) = widget_events().lock() {
            let len = events.len();
            if len > 0 {
                log::debug!("[rust_widgets] poll_widget_trigger_event: queue has {} events", len);
            }
            if let Some(event) = events.pop() {
                log::debug!(
                    "[rust_widgets] poll_widget_trigger_event: returning event for widget {}",
                    event.widget_id
                );
                return Some(event);
            }
        }
        // Fall back to state-based events
        self.state.pop_widget_trigger_event()
    }
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        self.state.inject_widget_trigger_event(widget_id, kind)
    }
    fn create_message_box(
        &self,
        _parent: ObjectId,
        title: &str,
        _text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.state.create_widget(HandleKind::MessageBox, title, x, y, width, height)
    }
    fn create_file_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.state.create_widget(HandleKind::FileDialog, "file_dialog", x, y, width, height)
    }
    fn create_color_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.state.create_widget(HandleKind::ColorDialog, "color_dialog", x, y, width, height)
    }
    fn create_font_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.state.create_widget(HandleKind::FontDialog, "font_dialog", x, y, width, height)
    }
    fn create_spin_box(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.state.create_widget(HandleKind::SpinBox, "spin_box", x, y, width, height)
    }
    fn create_list_view(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.state.create_widget(HandleKind::ListView, "list_view", x, y, width, height)
    }
    fn create_scroll_area(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.state.create_widget(HandleKind::ScrollArea, "scroll_area", x, y, width, height)
    }

    fn ime_bridge(&self) -> Option<&dyn ImeBridge> {
        Some(&self.ime_bridge)
    }

    fn clipboard_backend(&self) -> Option<&dyn RichClipboardBackend> {
        Some(&self.clipboard)
    }

    fn accessibility_bridge(&self) -> Option<&dyn AccessibilityBridge> {
        Some(&self.a11y_bridge)
    }
}
