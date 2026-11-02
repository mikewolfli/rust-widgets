// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Menu-event handles: menu bars, menus, items, tool bars and status bars.

use super::*;
use crate::core::{ObjectId, Rect};
use alloc::rc::Rc;
use core::cell::RefCell;

impl_handle!(MenuBarHandle, "Type-safe handle for a native menu bar.");
impl_handle!(MenuHandle, "Type-safe handle for a native menu (a menu-bar entry).");

impl MenuBarHandle {
    /// Subscribes to the title of an enabled entry the user clicked.
    ///
    /// `on_click` cannot serve this control: a menu bar has one target per entry, and its
    /// published event is `triggered` carrying the title, not `clicked` carrying nothing.
    /// The subscription is recorded like the other handle callbacks, so
    /// `WidgetHandle::remove_callbacks` really disconnects it.
    pub fn on_triggered<F: FnMut(String) + Send + 'static>(&self, f: F) {
        let id = self.raw_id();
        let held: std::sync::Arc<std::sync::Mutex<F>> =
            std::sync::Arc::new(std::sync::Mutex::new(f));
        let connected = crate::widget::runtime::with_widget_mut(id, |widget| {
            let held = std::sync::Arc::clone(&held);
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::menu_bar::MenuBar>()
                .map(|bar| {
                    // Borrowed slot: the callback needs an owned `String` anyway, so cloning
                    // here moves the copy from an `Arc` allocation per emit to one clone that
                    // the user callback would have performed regardless.
                    bar.triggered.connect_ref(move |title: &String| {
                        if let Ok(mut callback) = held.lock() {
                            callback(title.clone());
                        } else {
                            log::warn!(
                                "the menu-triggered callback for widget {id} could not be locked; \
                                 skipping this trigger rather than panicking"
                            );
                        }
                    })
                })
        })
        .flatten();
        match connected {
            Some(handle) => record_signal_connection(id, SignalKind::Triggered, handle),
            None => {
                log::warn!(
                    "on_triggered ignored for id={id}: it is not a live menu bar, so the callback \
                     will never run."
                );
                record_unwired_binding(id);
            }
        }
    }

    /// Subscribes to an entry click with the **entry rectangle** as well as its title.
    ///
    /// This is the form a host needs to open the matching drop-down under the clicked entry.
    /// Asking the bar for [`MenuBarHandle::entry_rect`] inside a plain `on_triggered` handler
    /// cannot work: the entry is dispatched with an exclusive loan, so a nested read answers
    /// `None`. The emitter therefore carries the rectangle in the event payload.
    pub fn on_entry_triggered<F: FnMut(String, Rect) + Send + 'static>(&self, f: F) {
        let id = self.raw_id();
        let held: std::sync::Arc<std::sync::Mutex<F>> =
            std::sync::Arc::new(std::sync::Mutex::new(f));
        let connected = crate::widget::runtime::with_widget_mut(id, |widget| {
            let held = std::sync::Arc::clone(&held);
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::menu_bar::MenuBar>()
                .map(|bar| {
                    bar.triggered_entry.connect_ref(move |payload: &(String, Rect)| {
                        if let Ok(mut callback) = held.lock() {
                            callback(payload.0.clone(), payload.1);
                        } else {
                            log::warn!(
                                "the menu-entry callback for widget {id} could not be locked; \
                                     skipping this trigger rather than panicking"
                            );
                        }
                    })
                })
        })
        .flatten();
        match connected {
            Some(handle) => record_signal_connection(id, SignalKind::TriggeredEntry, handle),
            None => {
                log::warn!(
                    "on_entry_triggered ignored for id={id}: it is not a live menu bar, so the \
                     callback will never run."
                );
                record_unwired_binding(id);
            }
        }
    }

    /// Subscribes to the **action id** of an entry the user clicked (DEMO-002).
    ///
    /// The companion of [`MenuBarHandle::on_entry_triggered`]. A host that decides which menu to
    /// open from that callback's title keeps a title→menu table beside the bar; naming the entry
    /// with [`MenuBarHandle::set_entry_action_id`] removes it, and the id is the same surface a
    /// drop-down row already carries. Entries with no id emit nothing, so a host that does not name
    /// them is unaffected.
    pub fn on_entry_action_triggered<F: FnMut(String, Rect) + Send + 'static>(&self, f: F) {
        let id = self.raw_id();
        let held: std::sync::Arc<std::sync::Mutex<F>> =
            std::sync::Arc::new(std::sync::Mutex::new(f));
        let connected = crate::widget::runtime::with_widget_mut(id, |widget| {
            let held = std::sync::Arc::clone(&held);
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::menu_bar::MenuBar>()
                .map(|bar| {
                    bar.entry_action_triggered.connect_ref(move |payload: &(String, Rect)| {
                        if let Ok(mut callback) = held.lock() {
                            callback(payload.0.clone(), payload.1);
                        } else {
                            log::warn!(
                                "the menu-entry action callback for widget {id} could not be \
                                     locked; skipping this trigger rather than panicking"
                            );
                        }
                    })
                })
        })
        .flatten();
        match connected {
            Some(handle) => record_signal_connection(id, SignalKind::TriggeredEntry, handle),
            None => {
                log::warn!(
                    "on_entry_action_triggered ignored for id={id}: it is not a live menu bar, so \
                     the callback will never run."
                );
                record_unwired_binding(id);
            }
        }
    }

    /// Names the entry at `index` with an action id, which
    /// [`MenuBarHandle::on_entry_action_triggered`] then reports. `None` clears it.
    ///
    /// Returns `false` for an out-of-range index or an id that is not a menu bar.
    pub fn set_entry_action_id(&self, index: usize, action_id: Option<String>) -> bool {
        crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::menu_bar::MenuBar>()
                .is_some_and(|bar| bar.set_entry_action_id(index, action_id.clone()))
        })
        .unwrap_or(false)
    }

    /// Number of top-level entries the bar holds.
    ///
    /// A bar created by [`WindowHandle::new_menu_bar`] starts empty; each
    /// [`WindowHandle::new_menu`] adds one entry, so this is also the check that a menu
    /// really reached the bar instead of only creating a popup widget.
    pub fn entry_count(&self) -> usize {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            (widget as &dyn core::any::Any)
                .downcast_ref::<crate::widget::menu_toolbar::menu_bar::MenuBar>()
                .map(|bar| bar.entries().len())
        })
        .flatten()
        .unwrap_or(0)
    }

    /// The rectangle of the entry at `index`, in client coordinates.
    ///
    /// This is what a caller needs to open the matching menu **under the entry the user
    /// clicked** rather than at a guessed offset. `None` for an out-of-range index or an
    /// id that is not a menu bar.
    pub fn entry_rect(&self, index: usize) -> Option<Rect> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            (widget as &dyn core::any::Any)
                .downcast_ref::<crate::widget::menu_toolbar::menu_bar::MenuBar>()
                .and_then(|bar| bar.entry_rect(index))
        })
        .flatten()
    }
}

impl MenuHandle {
    /// Subscribes to the label of the entry the user chose.
    ///
    /// A menu's published event is `triggered` with the entry text; `on_click` targets the
    /// base `clicked` signal, which a menu never emits, so this is the only handle-level
    /// way to hear a choice. The subscription is recorded for `remove_callbacks`.
    pub fn on_triggered<F: FnMut(String) + Send + 'static>(&self, f: F) {
        let id = self.raw_id();
        let held: std::sync::Arc<std::sync::Mutex<F>> =
            std::sync::Arc::new(std::sync::Mutex::new(f));
        let connected = crate::widget::runtime::with_widget_mut(id, |widget| {
            let held = std::sync::Arc::clone(&held);
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::menu::Menu>()
                .map(|menu| {
                    menu.triggered.connect_ref(move |title: &String| {
                        if let Ok(mut callback) = held.lock() {
                            callback(title.clone());
                        } else {
                            log::warn!(
                                "the menu-triggered callback for widget {id} could not be locked; \
                                 skipping this trigger rather than panicking"
                            );
                        }
                    })
                })
        })
        .flatten();
        match connected {
            Some(handle) => record_signal_connection(id, SignalKind::Triggered, handle),
            None => {
                log::warn!(
                    "on_triggered ignored for id={id}: it is not a live menu, so the callback will \
                     never run."
                );
                record_unwired_binding(id);
            }
        }
    }

    /// Number of entries, separators included.
    pub fn item_count(&self) -> usize {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            (widget as &dyn core::any::Any)
                .downcast_ref::<crate::widget::menu_toolbar::menu::Menu>()
                .map(|menu| menu.items().len())
        })
        .flatten()
        .unwrap_or(0)
    }

    /// Creates a **nested** menu under this menu (feedback C-4).
    ///
    /// A menu's parent is structurally the menu it hangs from, exactly as
    /// [`WindowHandle::new_menu`]'s parent is its bar. Both parents are menus to the backend: the
    /// popup is mounted in the overlay layer (a child of the top-level window) so it can extend
    /// past this menu's own rectangle, and the self-drawn menu model gains a branch row carrying
    /// the submenu indicator. The child is returned so the host can add items to it with
    /// [`WindowHandle::new_menu_item`].
    ///
    /// # Opening it
    ///
    /// The library does not guess where a submenu belongs while the pointer is still moving. When
    /// the branch row is activated this menu emits `submenu_requested` with the row's label; the
    /// host answers by calling [`MenuHandle::open_at`] on the returned handle, positioning it with
    /// [`MenuHandle::row_rect`]. That is the same division the rest of the menu API uses: the
    /// control owns its rows, the host owns when and where a popup is shown.
    pub fn new_submenu(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> MenuHandle {
        MenuHandle::from_raw(crate::create_menu(self.raw_id(), text, x, y, w, h))
    }

    /// The painted rectangle of row `index`, in window coordinates.
    ///
    /// This is what positions a nested menu ([`MenuHandle::new_submenu`]) under the branch row the
    /// user activated. `None` for an out-of-range row or a menu that is not mounted.
    pub fn row_rect(&self, index: usize) -> Option<Rect> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            (widget as &dyn core::any::Any)
                .downcast_ref::<crate::widget::menu_toolbar::menu::Menu>()
                .and_then(|menu| menu.row_rect(index))
        })
        .flatten()
    }

    /// Opens this menu with its top-left corner at `(x, y)` in client coordinates.
    ///
    /// The popup is clamped into `viewport` and made the window's last child, so it paints
    /// above the controls created after it. Returns `false` when the id is not a mounted
    /// menu.
    pub fn open_at(&self, x: i32, y: i32, viewport: Rect) -> bool {
        crate::menu_open_at(self.raw_id(), x, y, viewport)
    }

    /// Dismisses the popup. Returns `false` when the id is not a mounted menu.
    pub fn close(&self) -> bool {
        crate::menu_close(self.raw_id())
    }
}
impl_handle!(MenuItemHandle, "Type-safe handle for a native menu item.");
impl_handle!(ToolBarHandle, "Type-safe handle for a native tool bar.");

impl ToolBarHandle {
    /// Replaces the label of item `index`, returning whether the change landed.
    ///
    /// The handle form of [`crate::widget::menu_toolbar::tool_bar::ToolBar::set_item_text`]: an i18n
    /// reload can relabel a strip without
    /// rebuilding it, keeping every action id, checked state and action link in place (feedback
    /// C-5). `false` for an out-of-range index or an id that is not a live tool bar.
    pub fn set_item_text(&self, index: usize, text: &str) -> bool {
        crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                .is_some_and(|bar| bar.set_item_text(index, text))
        })
        .unwrap_or(false)
    }

    /// The number of action items, or `0` when the id is not a live tool bar (RW-63).
    pub fn item_count(&self) -> usize {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            (widget as &dyn core::any::Any)
                .downcast_ref::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                .map(|bar| bar.items().len())
                .unwrap_or(0)
        })
        .unwrap_or(0)
    }

    /// Appends an action item and returns its index (RW-63).
    ///
    /// `None` when the id is not a live tool bar. The typed entry already repaints.
    pub fn add_action(&self, id: &str, text: &str) -> Option<usize> {
        crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                .map(|bar| bar.add_action(id, text))
        })
        .flatten()
    }

    /// Removes the item at `index`, returning whether it existed (RW-63).
    pub fn remove_item(&self, index: usize) -> bool {
        crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                .is_some_and(|bar| bar.remove_item(index).is_some())
        })
        .unwrap_or(false)
    }

    /// Removes every action item, returning whether the id is a live tool bar (RW-63).
    pub fn clear(&self) -> bool {
        crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                .is_some_and(|bar| {
                    bar.clear();
                    true
                })
        })
        .unwrap_or(false)
    }

    /// Enables or disables item `index`, returning whether the id is a live tool bar (RW-63).
    pub fn set_item_enabled(&self, index: usize, enabled: bool) -> bool {
        let in_range = self.item_count() > index;
        if in_range {
            crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
                (widget as &mut dyn core::any::Any)
                    .downcast_mut::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                    .map(|bar| bar.set_item_enabled(index, enabled))
            });
        }
        in_range
    }

    /// Whether item `index` is enabled, or `None` for an unknown index (RW-63).
    pub fn item_enabled(&self, index: usize) -> Option<bool> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            (widget as &dyn core::any::Any)
                .downcast_ref::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                .and_then(|bar| bar.item_enabled(index))
        })
        .flatten()
    }

    /// Sets item `index`'s checked state, returning whether the id is a live tool bar (RW-63).
    pub fn set_item_checked(&self, index: usize, checked: bool) -> bool {
        let in_range = self.item_count() > index;
        if in_range {
            crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
                (widget as &mut dyn core::any::Any)
                    .downcast_mut::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                    .map(|bar| bar.set_item_checked(index, checked))
            });
        }
        in_range
    }

    /// Whether item `index` is checked, or `None` for an unknown index (RW-63).
    pub fn item_checked(&self, index: usize) -> Option<bool> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            (widget as &dyn core::any::Any)
                .downcast_ref::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                .and_then(|bar| bar.item_checked(index))
        })
        .flatten()
    }

    /// Sets item `index`'s hover hint, returning whether the write landed (RW-63).
    pub fn set_item_tooltip(&self, index: usize, text: &str) -> bool {
        crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                .and_then(|bar| bar.item_mut(index))
                .map(|item| item.set_tooltip(text))
                .is_some()
        })
        .unwrap_or(false)
    }

    /// Sets the strip's orientation, returning whether the id is a live tool bar (RW-63).
    pub fn set_orientation(
        &self,
        orientation: crate::widget::menu_toolbar::tool_bar::ToolBarOrientation,
    ) -> bool {
        crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                .map(|bar| bar.set_orientation(orientation))
        })
        .is_some()
    }

    /// Sets the strip's icon size, returning whether the id is a live tool bar (RW-63).
    pub fn set_icon_size(&self, size: f32) -> bool {
        crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::tool_bar::ToolBar>()
                .map(|bar| bar.set_icon_size(size))
        })
        .is_some()
    }
}
impl_handle!(StatusBarHandle, "Type-safe handle for a native status bar.");
