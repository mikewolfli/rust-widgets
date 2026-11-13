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
        let held: std::sync::Arc<std::sync::Mutex<Option<F>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(f)));
        let connected = crate::widget::runtime::with_widget_mut(id, |widget| {
            let held = std::sync::Arc::clone(&held);
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::menu_bar::MenuBar>()
                .map(|bar| {
                    // Borrowed slot: the callback needs an owned `String` anyway, so cloning
                    // here moves the copy from an `Arc` allocation per emit to one clone that
                    // the user callback would have performed regardless.
                    bar.triggered.connect_ref(move |title: &String| {
                        run_shared_callback(&held, "menu-triggered", id, |callback| {
                            callback(title.clone())
                        });
                    })
                })
        })
        .flatten();
        match connected {
            Some(handle) => record_signal_connection(id, SignalKind::MenuBarTriggered, handle),
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
        let held: std::sync::Arc<std::sync::Mutex<Option<F>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(f)));
        let connected = crate::widget::runtime::with_widget_mut(id, |widget| {
            let held = std::sync::Arc::clone(&held);
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::menu_bar::MenuBar>()
                .map(|bar| {
                    bar.triggered_entry.connect_ref(move |payload: &(String, Rect)| {
                        run_shared_callback(&held, "menu-entry", id, |callback| {
                            callback(payload.0.clone(), payload.1)
                        });
                    })
                })
        })
        .flatten();
        match connected {
            Some(handle) => record_signal_connection(id, SignalKind::MenuBarTriggeredEntry, handle),
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
        let held: std::sync::Arc<std::sync::Mutex<Option<F>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(f)));
        let connected = crate::widget::runtime::with_widget_mut(id, |widget| {
            let held = std::sync::Arc::clone(&held);
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::menu_bar::MenuBar>()
                .map(|bar| {
                    bar.entry_action_triggered.connect_ref(move |payload: &(String, Rect)| {
                        run_shared_callback(&held, "menu-entry action", id, |callback| {
                            callback(payload.0.clone(), payload.1)
                        });
                    })
                })
        })
        .flatten();
        match connected {
            Some(handle) => record_signal_connection(id, SignalKind::MenuBarEntryAction, handle),
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
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
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
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
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
        let held: std::sync::Arc<std::sync::Mutex<Option<F>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(f)));
        let connected = crate::widget::runtime::with_widget_mut(id, |widget| {
            let held = std::sync::Arc::clone(&held);
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::menu::Menu>()
                .map(|menu| {
                    menu.triggered.connect_ref(move |title: &String| {
                        run_shared_callback(&held, "menu-triggered", id, |callback| {
                            callback(title.clone())
                        });
                    })
                })
        })
        .flatten();
        match connected {
            Some(handle) => record_signal_connection(id, SignalKind::MenuTriggered, handle),
            None => {
                log::warn!(
                    "on_triggered ignored for id={id}: it is not a live menu, so the callback will \
                     never run."
                );
                record_unwired_binding(id);
            }
        }
    }

    /// Subscribes to the **action id** of a row the user chose (DM3-2).
    ///
    /// The row-level counterpart of [`MenuBarHandle::on_entry_action_triggered`]. A row's id is
    /// what a host dispatches on; the label is presentation and may change without warning. Rows
    /// with no id emit nothing here, so a menu whose rows are plain labels keeps behaving exactly
    /// as before.
    ///
    /// This is the handle-level path to the menu's own `action_triggered` signal, which fires for
    /// any row carrying an id — including one named through
    /// [`WindowHandle::new_menu_item_with_action_id`].
    pub fn on_item_action_triggered<F: FnMut(String) + Send + 'static>(&self, f: F) {
        let id = self.raw_id();
        let held: std::sync::Arc<std::sync::Mutex<Option<F>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(f)));
        let connected = crate::widget::runtime::with_widget_mut(id, |widget| {
            let held = std::sync::Arc::clone(&held);
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::menu::Menu>()
                .map(|menu| {
                    menu.action_triggered.connect_ref(move |action_id: &String| {
                        run_shared_callback(&held, "menu-item action", id, |callback| {
                            callback(action_id.clone())
                        });
                    })
                })
        })
        .flatten();
        match connected {
            Some(handle) => record_signal_connection(id, SignalKind::MenuItemAction, handle),
            None => {
                log::warn!(
                    "on_item_action_triggered ignored for id={id}: it is not a live menu, so the \
                     callback will never run."
                );
                record_unwired_binding(id);
            }
        }
    }

    /// Names the row at `index` with an action id, or clears it with `None`.
    ///
    /// Returns `false` for an out-of-range row or an id that is not a live menu. The id is what
    /// [`MenuHandle::on_item_action_triggered`] reports when that row is chosen; it does not
    /// change the row's label or shortcut.
    pub fn set_item_action_id(&self, index: usize, action_id: Option<String>) -> bool {
        crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::menu_toolbar::menu::Menu>()
                .is_some_and(|menu| menu.set_item_action_id(index, action_id.clone()))
        })
        .unwrap_or(false)
    }

    /// Number of entries, separators included.
    pub fn item_count(&self) -> usize {
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
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
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
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

impl WindowHandle {
    /// Add a menu item carrying an **action id** for id-based dispatch.
    ///
    /// The sibling of [`WindowHandle::new_menu_item`]: it creates the row and links it with
    /// [`MenuHandle::set_item_action_id`], so choosing it reports the id through
    /// [`MenuHandle::on_item_action_triggered`] instead of leaving the visible label as the only
    /// routing key (DM3-2). `action_id` is optional; `None` creates exactly the row
    /// [`WindowHandle::new_menu_item`] would.
    ///
    /// `shortcut` is display text, like [`WindowHandle::new_menu_item`]'s.
    ///
    /// # Why this is a sibling rather than a fourth parameter
    ///
    /// [`WindowHandle::new_menu_item`] is published API; adding a parameter would break every
    /// host that already calls it. This is the same extension shape as
    /// [`WindowHandle::new_menu_item_with_shortcut`] and
    /// [`WindowHandle::new_menu_item_with_icon`].
    pub fn new_menu_item_with_action_id(
        &self,
        menu: &MenuHandle,
        text: &str,
        shortcut: Option<&str>,
        action_id: Option<&str>,
    ) -> MenuItemHandle {
        // The menu appends its rows, so the count before the call is the index the new row
        // lands on. Reading it first also means a row the backend refused is not named: those
        // calls return id `0` and leave the list unchanged.
        let index = menu.item_count();
        let item = MenuItemHandle::from_raw(crate::menu_add_item(menu.raw_id(), text, shortcut));
        if let Some(action_id) = action_id {
            if item.raw_id() == 0 {
                log::warn!(
                    "menu item {text:?} could not be created, so action id {action_id:?} names \
                     no row"
                );
            } else if !menu.set_item_action_id(index, Some(action_id.to_string())) {
                log::warn!(
                    "menu item {text:?} was created, but action id {action_id:?} could not be \
                     linked to row {index}; dispatch by id will not fire for it"
                );
            }
        }
        item
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
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
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
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
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
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
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
