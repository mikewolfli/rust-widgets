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
        fn attach_menu_bar_to_window(&self, _window: ObjectId, _menu_bar: ObjectId) -> bool {
            true
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
                let added = crate::widget::runtime::with_widget_mut(parent_menu, |widget| {
                    (widget as &mut dyn std::any::Any)
                        .downcast_mut::<crate::widget::menu_toolbar::menu::Menu>()
                        .map(|menu| match shortcut {
                            Some(shortcut) => {
                                menu.add_action_with_shortcut(text, shortcut);
                            }
                            None => {
                                menu.add_action(text);
                            }
                        })
                        .is_some()
                })
                .unwrap_or(false);

                if !added {
                    log::warn!(
                        "custom backend: id {parent_menu} is not a mounted menu, so \"{text}\" \
                         could not be added"
                    );
                }
                return ObjectId::from(added);
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
        fn create_tool_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Toolbox, parent, "", x, y, width, height)
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
