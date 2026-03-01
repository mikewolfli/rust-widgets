//! Linux backend shell.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use crate::core::PlatformFamily;

use super::{Platform, StubPlatform, WidgetTriggerEvent, WidgetTriggerKind};

#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use gtk::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LinuxHandleKind {
    Window,
    Button,
    CheckBox,
    LineEdit,
    MenuBar,
    Menu,
    MenuItem,
    ToolBar,
    StatusBar,
}

#[derive(Default)]
struct LinuxMenuState {
    /// Tracks menu bar attachment by window id.
    attached_menu_bar: HashMap<u64, u64>,
    /// Maintains menu tree relationships.
    menu_children: HashMap<u64, Vec<u64>>,
    /// Parent lookup for geometry updates in gtk-native fixed containers.
    widget_parent: HashMap<u64, u64>,
    /// FIFO queue for menu triggers.
    pending_menu_events: VecDeque<u64>,
    /// FIFO queue for typed widget triggers.
    pending_widget_events: VecDeque<WidgetTriggerEvent>,
}

pub struct LinuxPlatform {
    inner: StubPlatform,
    kinds: Mutex<HashMap<u64, LinuxHandleKind>>,
    menus: Arc<Mutex<LinuxMenuState>>,
    #[cfg(all(target_os = "linux", feature = "gtk-native"))]
    native: Mutex<LinuxNativeState>,
}

#[cfg(all(target_os = "linux", feature = "gtk-native"))]
#[derive(Default)]
struct LinuxNativeState {
    /// Native GTK windows indexed by logical widget id.
    windows: HashMap<u64, gtk::Window>,
    /// Root vertical containers hosting menu bar and content area.
    root_boxes: HashMap<u64, gtk::Box>,
    /// Absolute-position container for child controls.
    content_fixed: HashMap<u64, gtk::Fixed>,
    /// Generic widget registry for visibility/text/enabled operations.
    widgets: HashMap<u64, gtk::Widget>,
    menu_bars: HashMap<u64, gtk::MenuBar>,
    menus: HashMap<u64, gtk::Menu>,
}

impl LinuxPlatform {
    pub fn new() -> Self {
        Self {
            inner: StubPlatform::new("gtk", PlatformFamily::Desktop),
            kinds: Mutex::new(HashMap::new()),
            menus: Arc::new(Mutex::new(LinuxMenuState::default())),
            #[cfg(all(target_os = "linux", feature = "gtk-native"))]
            native: Mutex::new(LinuxNativeState::default()),
        }
    }

    fn set_kind(&self, id: u64, kind: LinuxHandleKind) {
        self.kinds
            .lock()
            .expect("linux kind lock poisoned")
            .insert(id, kind);
    }

    fn kind_of(&self, id: u64) -> Option<LinuxHandleKind> {
        self.kinds
            .lock()
            .expect("linux kind lock poisoned")
            .get(&id)
            .copied()
    }

}

impl Platform for LinuxPlatform {
    fn backend_name(&self) -> &'static str { self.inner.backend_name() }
    fn family(&self) -> PlatformFamily { self.inner.family() }
    fn init(&self) {
        self.inner.init();
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // Initialize GTK runtime when native path is enabled.
            let _ = gtk::init();
        }
    }
    fn run(&self) {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // Enter GTK event loop for native-backed Linux runtime.
            gtk::main();
            return;
        }
        self.inner.run();
    }
    fn quit(&self) {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            gtk::main_quit();
        }
        self.inner.quit();
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_window(title, x, y, width, height);
        self.set_kind(id, LinuxHandleKind::Window);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title(title);
            window.set_default_size(width as i32, height as i32);
            window.move_(x, y);

            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let fixed = gtk::Fixed::new();
            root.pack_start(&fixed, true, true, 0);
            window.add(&root);

            let mut native = self.native.lock().expect("linux native lock poisoned");
            native.windows.insert(id, window.clone());
            native.root_boxes.insert(id, root);
            native.content_fixed.insert(id, fixed.clone());
            native.widgets.insert(id, window.clone().upcast::<gtk::Widget>());
        }
        id
    }

    fn create_button(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_button(parent, text, x, y, width, height);
        self.set_kind(id, LinuxHandleKind::Button);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let button = gtk::Button::with_label(text);
            button.set_size_request(width as i32, height as i32);

            let menus = Arc::clone(&self.menus);
            button.connect_clicked(move |_| {
                // Normalize native button activation to typed trigger event.
                menus
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_widget_events
                    .push_back(WidgetTriggerEvent {
                        widget_id: id,
                        kind: WidgetTriggerKind::Clicked,
                    });
            });

            let widget = button.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&button, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    fn create_checkbox(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_checkbox(parent, text, x, y, width, height);
        self.set_kind(id, LinuxHandleKind::CheckBox);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let checkbox = gtk::CheckButton::with_label(text);
            checkbox.set_size_request(width as i32, height as i32);

            let menus = Arc::clone(&self.menus);
            checkbox.connect_toggled(move |_| {
                // Normalize checkbox toggles to click-like activation trigger.
                menus
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_widget_events
                    .push_back(WidgetTriggerEvent {
                        widget_id: id,
                        kind: WidgetTriggerKind::Clicked,
                    });
            });

            let widget = checkbox.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&checkbox, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    fn create_line_edit(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_line_edit(parent, text, x, y, width, height);
        self.set_kind(id, LinuxHandleKind::LineEdit);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let entry = gtk::Entry::new();
            entry.set_text(text);
            entry.set_size_request(width as i32, height as i32);

            let menus = Arc::clone(&self.menus);
            entry.connect_changed(move |_| {
                // Normalize text changes to value-changed trigger.
                menus
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_widget_events
                    .push_back(WidgetTriggerEvent {
                        widget_id: id,
                        kind: WidgetTriggerKind::ValueChanged,
                    });
            });

            let widget = entry.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&entry, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_menu_bar(parent, x, y, width, height);
        self.set_kind(id, LinuxHandleKind::MenuBar);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu_bar = gtk::MenuBar::new();
            let widget = menu_bar.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            native.menu_bars.insert(id, menu_bar);
            native.widgets.insert(id, widget);
            let _ = parent;
            let _ = (x, y, width, height);
        }
        id
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_menu(parent, text, x, y, width, height);
        self.set_kind(id, LinuxHandleKind::Menu);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .menu_children
            .entry(parent)
            .or_default()
            .push(id);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu = gtk::Menu::new();
            let menu_item = gtk::MenuItem::with_label(text);
            menu_item.set_submenu(Some(&menu));

            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(menu_bar) = native.menu_bars.get(&parent) {
                menu_bar.append(&menu_item);
            } else if let Some(parent_menu) = native.menus.get(&parent) {
                parent_menu.append(&menu_item);
            }
            native.widgets.insert(id, menu_item.clone().upcast::<gtk::Widget>());
            native.menus.insert(id, menu);

            let _ = (x, y, width, height);
        }
        id
    }

    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_tool_bar(parent, x, y, width, height);
        self.set_kind(id, LinuxHandleKind::ToolBar);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            toolbar.set_size_request(width as i32, height as i32);
            let widget = toolbar.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&toolbar, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    fn create_status_bar(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_status_bar(parent, text, x, y, width, height);
        self.set_kind(id, LinuxHandleKind::StatusBar);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let label = gtk::Label::new(Some(text));
            label.set_size_request(width as i32, height as i32);
            let widget = label.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&label, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        let attached = self.inner.attach_menu_bar_to_window(window, menu_bar);
        // Validate shape first, then attach native menu bar when available.
        if matches!(self.kind_of(window), Some(LinuxHandleKind::Window))
            && matches!(self.kind_of(menu_bar), Some(LinuxHandleKind::MenuBar))
        {
            self.menus
                .lock()
                .expect("linux menu lock poisoned")
                .attached_menu_bar
                .insert(window, menu_bar);
            #[cfg(all(target_os = "linux", feature = "gtk-native"))]
            {
                let native = self.native.lock().expect("linux native lock poisoned");
                if let (Some(root), Some(bar)) = (native.root_boxes.get(&window), native.menu_bars.get(&menu_bar)) {
                    root.pack_start(bar, false, false, 0);
                    root.reorder_child(bar, 0);
                    bar.show_all();
                }
            }
            return true;
        }
        attached
    }

    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        let item_id = self.inner.menu_add_item(parent_menu, text, shortcut);
        self.set_kind(item_id, LinuxHandleKind::MenuItem);
        let mut menus = self.menus.lock().expect("linux menu lock poisoned");
        menus.menu_children.entry(parent_menu).or_default().push(item_id);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu_item = gtk::MenuItem::with_label(text);
            let menus_arc = Arc::clone(&self.menus);
            menu_item.connect_activate(move |_| {
                // Native menu activation is forwarded to platform queue.
                menus_arc
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_menu_events
                    .push_back(item_id);
            });

            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(parent) = native.menus.get(&parent_menu) {
                parent.append(&menu_item);
            }
            native
                .widgets
                .insert(item_id, menu_item.clone().upcast::<gtk::Widget>());
        }
        item_id
    }

    fn poll_menu_triggered(&self) -> Option<u64> {
        let mut menus = self.menus.lock().expect("linux menu lock poisoned");
        if let Some(item_id) = menus.pending_menu_events.pop_front() {
            return Some(item_id);
        }
        drop(menus);
        self.inner.poll_menu_triggered()
    }

    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        // Keep injected events type-safe: only known menu items are accepted.
        if !matches!(self.kind_of(menu_item_id), Some(LinuxHandleKind::MenuItem)) {
            return false;
        }
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .pending_menu_events
            .push_back(menu_item_id);
        true
    }

    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        let mut menus = self.menus.lock().expect("linux menu lock poisoned");
        if let Some(event) = menus.pending_widget_events.pop_front() {
            return Some(event);
        }
        drop(menus);
        self.inner.poll_widget_trigger_event()
    }

    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        // Keep injected events deterministic: only known widget ids are accepted.
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .pending_widget_events
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }

    fn show_widget(&self, widget_id: u64) {
        self.inner.show_widget(widget_id);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(window) = native.windows.get(&widget_id) {
                window.show_all();
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.show();
            }
        }
    }

    fn hide_widget(&self, widget_id: u64) {
        self.inner.hide_widget(widget_id);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(window) = native.windows.get(&widget_id) {
                window.hide();
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.hide();
            }
        }
    }

    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.inner.set_widget_geometry(widget_id, x, y, width, height);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let parent_id = self
                .menus
                .lock()
                .expect("linux menu lock poisoned")
                .widget_parent
                .get(&widget_id)
                .copied();
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(window) = native.windows.get(&widget_id) {
                window.move_(x, y);
                window.resize(width as i32, height as i32);
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.set_size_request(width as i32, height as i32);
                if let Some(parent_id) = parent_id {
                    if let Some(container) = native.content_fixed.get(&parent_id) {
                        container.move_(widget, x, y);
                    }
                }
            }
        }
    }

    fn set_widget_text(&self, widget_id: u64, text: &str) {
        self.inner.set_widget_text(widget_id, text);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(window) = native.windows.get(&widget_id) {
                window.set_title(text);
            } else if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
                    button.set_label(text);
                } else if let Ok(check) = widget.clone().downcast::<gtk::CheckButton>() {
                    check.set_label(text);
                } else if let Ok(entry) = widget.clone().downcast::<gtk::Entry>() {
                    entry.set_text(text);
                } else if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
                    label.set_text(text);
                } else if let Ok(menu_item) = widget.clone().downcast::<gtk::MenuItem>() {
                    menu_item.set_label(text);
                }
            }
        }
        if matches!(self.kind_of(widget_id), Some(LinuxHandleKind::LineEdit)) {
            self.menus
                .lock()
                .expect("linux menu lock poisoned")
                .pending_widget_events
                .push_back(WidgetTriggerEvent {
                    widget_id,
                    kind: WidgetTriggerKind::ValueChanged,
                });
        }
    }

    fn get_widget_text(&self, widget_id: u64) -> String { self.inner.get_widget_text(widget_id) }

    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.inner.set_widget_enabled(widget_id, enabled);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.set_sensitive(enabled);
            }
        }
    }

    fn is_widget_enabled(&self, widget_id: u64) -> bool {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(widget) = native.widgets.get(&widget_id) {
                return widget.is_sensitive();
            }
        }
        self.inner.is_widget_enabled(widget_id)
    }

    fn set_widget_visible(&self, widget_id: u64, visible: bool) {
        self.inner.set_widget_visible(widget_id, visible);
        if visible {
            self.show_widget(widget_id);
        } else {
            self.hide_widget(widget_id);
        }
    }

    fn is_widget_visible(&self, widget_id: u64) -> bool {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(window) = native.windows.get(&widget_id) {
                return window.is_visible();
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                return widget.is_visible();
            }
        }
        self.inner.is_widget_visible(widget_id)
    }
}
