// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

macro_rules! impl_menu_widgets {
    () => {
        #[cfg(not(alloc_frugal))]
        fn create_menu_bar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::MenuBar, parent, "", x, y, width, height)
        }
        fn create_menu(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                crate::control_backend::custom::MENU_KIND,
                parent,
                text,
                x,
                y,
                width,
                height,
            )
        }
        /// Attaches a menu bar to the window that owns it.
        ///
        /// There is no host menu to attach: the menu bar is a self-drawn widget, so
        /// "attaching" means making it a child of the window it belongs to. Both ids
        /// are checked first — the previous body returned `true` unconditionally,
        /// which claimed success for ids that addressed nothing.
        fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
            #[cfg(not(alloc_frugal))]
            {
                if !crate::widget::runtime::is_mounted(window)
                    || !crate::widget::runtime::is_mounted(menu_bar)
                {
                    return false;
                }
                let attached = crate::widget::runtime::with_widget_mut(menu_bar, |bar| {
                    bar.set_parent(Some(window))
                })
                .is_some();
                if attached {
                    crate::widget::runtime::request_repaint(window);
                }
                return attached;
            }
            #[cfg(alloc_frugal)]
            {
                let _ = (window, menu_bar);
                false
            }
        }
        /// Adds an entry to a menu and returns the menu's id.
        ///
        /// # Why this returns the menu id, not a new widget id
        ///
        /// A menu entry is a `MenuEntry` value inside `Menu`'s own `items` vector,
        /// not a mounted widget — see `widget::menu_toolbar::menu::Menu`. The old
        /// implementation minted a fresh id from a counter and stored a text string
        /// against it, so the id addressed nothing and the entry was invisible in
        /// every respect except that one map. Returning the menu's id keeps the
        /// contract honest: the call succeeded, and the thing it acted on is the
        /// menu the caller already holds.
        ///
        /// Returns `0` when `parent_menu` is not a mounted `Menu`, rather than
        /// silently succeeding.
        fn menu_add_item(
            &self,
            parent_menu: ObjectId,
            text: &str,
            shortcut: Option<&str>,
        ) -> ObjectId {
            // `widget::runtime` and `widget::menu_toolbar` are compiled out of the
            // alloc-frugal profile, so a menu entry cannot be stored there. Report
            // that honestly rather than pretending the call succeeded.
            #[cfg(alloc_frugal)]
            {
                let _ = (parent_menu, text, shortcut);
                log::warn!(
                    "custom backend: menu items are unavailable in this profile (no widget \
                     registry is compiled in)"
                );
                return 0;
            }
            #[cfg(all(not(alloc_frugal), full_widgets))]
            {
                let index = crate::widget::runtime::with_widget_mut(parent_menu, |widget| {
                    (widget as &mut dyn std::any::Any)
                        .downcast_mut::<crate::widget::menu_toolbar::menu::Menu>()
                        .map(|menu| match shortcut {
                            Some(shortcut) => menu.add_action_with_shortcut(text, shortcut),
                            None => menu.add_action(text),
                        })
                })
                .flatten();

                let Some(index) = index else {
                    log::warn!(
                        "custom backend: id {parent_menu} is not a mounted menu, so \"{text}\" \
                         could not be added"
                    );
                    return 0;
                };

                // A menu entry is a row inside the menu, not a widget, so it has no
                // id from the widget registry. Handing out one of this backend's
                // own — and remembering which row it names — is what lets a
                // trigger, or `menu_item_shortcut`, talk about the entry the caller
                // just added. The previous `ObjectId::from(added)` returned `1` for
                // every item: it addressed no entry at all, and collided with the
                // first id the platform hands out.
                let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                let id = super::super::types::allocate_menu_entry_id(&mut state);
                state.menu_entries.insert(id, (parent_menu, index));
                id
            }
            // `menu_toolbar` is compiled only where the full widget set is, so this
            // profile has no `Menu` type to downcast to. Reporting the failure is
            // the honest answer and keeps the method total in every profile.
            #[cfg(all(not(alloc_frugal), not(full_widgets)))]
            {
                let _ = (parent_menu, text, shortcut);
                log::warn!(
                    "custom backend: menus are unavailable in this profile (the menu widgets are \
                     not compiled in)"
                );
                0
            }
        }

        /// Reads a menu entry's accelerator text from the entry it names.
        fn menu_item_shortcut(&self, menu_item: ObjectId) -> Option<String> {
            #[cfg(all(not(alloc_frugal), full_widgets))]
            {
                let (menu_id, index) = {
                    let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    *state.menu_entries.get(&menu_item)?
                };
                return crate::widget::runtime::with_widget(menu_id, |widget| {
                    (widget as &dyn std::any::Any)
                        .downcast_ref::<crate::widget::menu_toolbar::menu::Menu>()
                        .and_then(|menu| menu.items().get(index))
                        .map(|entry| entry.shortcut().to_string())
                        .filter(|shortcut| !shortcut.is_empty())
                })
                .flatten();
            }
            #[cfg(any(alloc_frugal, not(full_widgets)))]
            {
                let _ = menu_item;
                None
            }
        }
        #[cfg(not(alloc_frugal))]
        fn create_tool_bar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ToolBar, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_status_bar(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::StatusBar, parent, text, x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_action(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Action, parent, text, x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_tool_button(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ToolButton, parent, text, x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        #[cfg(widgets_unstripped)]
        fn create_tool_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            #[cfg(full_widgets)]
            {
                self.mount_named_widget("tool_box", parent, "", x, y, width, height)
            }
            #[cfg(not(full_widgets))]
            {
                self.mount_widget_of_kind(WidgetKind::Toolbox, parent, "", x, y, width, height)
            }
        }
        #[cfg(not(alloc_frugal))]
        fn create_context_menu(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ContextMenu, parent, text, x, y, width, height)
        }

        fn poll_menu_triggered(&self) -> Option<ObjectId> {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .menu_trigger_queue
                .pop_front()
        }
        fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.menu_trigger_queue.push_back(menu_item_id);
            true
        }
    };
}
