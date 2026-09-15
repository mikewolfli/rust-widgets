// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

/// Runs `f` against the live widget at `widget_id` when it is a `T`.
///
/// List and combo-box item operations are *actions*, not state writes, so they
/// cannot travel through the property contract and need the concrete control. The
/// downcast is the capability layer's own helper — the same one the property hooks
/// use — so this adds no second type table (rule #67).
///
/// Returns `None` when the id addresses nothing or addresses a different kind,
/// which is what lets each caller report "this control has no items" instead of a
/// silent `true`.
#[cfg(not(alloc_frugal))]
fn with_typed_widget<T, R>(widget_id: ObjectId, f: impl FnOnce(&mut T) -> R) -> Option<R>
where
    T: crate::widget::Widget + 'static,
{
    crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        crate::widget::capability::widget_as_mut::<T>(widget).map(f)
    })
    .flatten()
}

macro_rules! impl_helpers {
    () => {
        fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .widget_trigger_queue
                .pop_front()
        }

        /// Pops the next triggered widget id.
        ///
        /// Shares one queue with `poll_widget_trigger_event` — they are two views
        /// of the same event stream — so both consume an event when they answer,
        /// exactly as the platform backends do.
        fn poll_widget_triggered(&self) -> Option<ObjectId> {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .widget_trigger_queue
                .pop_front()
                .map(|event| event.widget_id)
        }
        fn inject_widget_trigger_event(
            &self,
            widget_id: ObjectId,
            kind: WidgetTriggerKind,
        ) -> bool {
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.widget_trigger_queue.push_back(WidgetTriggerEvent { widget_id, kind });
            true
        }

        /// Writes the control's label, repainting it.
        ///
        /// Kinds disagree on the property's name — a `Button` exposes `text`, a
        /// `Window`/`GroupBox` exposes `title`, a `StatusBar` exposes `message`.
        /// Probing the known spellings keeps this accessor total without a `match`
        /// on kind, which would be the kind of central table this refactor removes.
        fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
            #[cfg(widgets_unstripped)]
            {
                let value = || crate::widget::capability::CapabilityValue::String(text.to_string());
                let mut written =
                    Err(crate::widget::capability::CapabilityAccessError::UnknownWidget);
                for property in LABEL_PROPERTY_NAMES {
                    match crate::widget::capability::write_widget_property_by_id(
                        widget_id,
                        property,
                        value(),
                    ) {
                        Ok(()) => {
                            written = Ok(());
                            break;
                        }
                        Err(error) => written = Err(error),
                    }
                }
                if written.is_err() {
                    log::warn!(
                        "custom backend: widget {widget_id} exposes none of \
                         {LABEL_PROPERTY_NAMES:?}, so the label could not be set"
                    );
                }
            }
            // A stripped profile has no property registry, so a label cannot be
            // written there. Reporting that is more useful than silently doing
            // nothing, and it matches the profile's whole purpose.
            #[cfg(not(widgets_unstripped))]
            {
                let _ = (widget_id, text);
                log::warn!(
                    "custom backend: labels are unavailable in this profile (no property \
                     registry is compiled in)"
                );
            }
        }

        /// Reads the control's label, trying each known spelling.
        fn get_widget_text(&self, widget_id: ObjectId) -> String {
            #[cfg(widgets_unstripped)]
            {
                for property in LABEL_PROPERTY_NAMES {
                    if let Ok(crate::widget::capability::CapabilityValue::String(text)) =
                        crate::widget::capability::read_widget_property_by_id(widget_id, property)
                    {
                        if !text.is_empty() {
                            return text;
                        }
                    }
                }
            }
            #[cfg(not(widgets_unstripped))]
            let _ = widget_id;
            String::new()
        }

        fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
            // `widget::runtime` is compiled out of the alloc-frugal profile, which
            // holds no widget objects by design. The host-side maps remain, so the
            // accessors degrade rather than fail to compile.
            #[cfg(not(alloc_frugal))]
            {
                self.with_live_widget(widget_id, |widget| widget.set_enabled(enabled));
                crate::widget::runtime::request_repaint(widget_id);
            }
            #[cfg(alloc_frugal)]
            let _ = (widget_id, enabled);
        }

        fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
            #[cfg(not(alloc_frugal))]
            {
                return self
                    .with_live_widget(widget_id, |widget| widget.is_enabled())
                    .unwrap_or(false);
            }
            #[cfg(alloc_frugal)]
            {
                let _ = widget_id;
                false
            }
        }

        fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
            #[cfg(not(alloc_frugal))]
            {
                self.with_live_widget(widget_id, |widget| widget.set_visible(visible));
                crate::widget::runtime::request_repaint(widget_id);
            }
            #[cfg(alloc_frugal)]
            let _ = (widget_id, visible);
        }

        fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
            #[cfg(not(alloc_frugal))]
            {
                return self
                    .with_live_widget(widget_id, |widget| widget.is_visible())
                    .unwrap_or(false);
            }
            #[cfg(alloc_frugal)]
            {
                let _ = widget_id;
                false
            }
        }

        /// Shows the control; the id-addressed spelling of `set_widget_visible`.
        fn show_widget(&self, widget_id: ObjectId) {
            self.set_widget_visible(widget_id, true);
        }

        /// Hides the control.
        fn hide_widget(&self, widget_id: ObjectId) {
            self.set_widget_visible(widget_id, false);
        }

        /// Reads the control's rectangle from the widget registry.
        ///
        /// The widget owns its geometry — the backend writes it into
        /// `widget::runtime` whenever it moves one — so the answer comes from there
        /// rather than from a backend-side copy that could disagree.
        fn get_widget_geometry(&self, widget_id: ObjectId) -> Option<(i32, i32, u32, u32)> {
            #[cfg(not(alloc_frugal))]
            {
                return crate::widget::runtime::geometry_of(widget_id)
                    .map(|rect| (rect.x, rect.y, rect.width, rect.height));
            }
            #[cfg(alloc_frugal)]
            {
                let _ = widget_id;
                None
            }
        }

        /// Appends an item to a combo box, repainting it.
        fn combo_box_add_item(&self, widget_id: ObjectId, text: &str) -> bool {
            #[cfg(not(alloc_frugal))]
            {
                let added =
                    with_typed_widget::<crate::widget::input_widgets::combobox::ComboBox, _>(
                        widget_id,
                        |combo| combo.add_item(text.to_string()),
                    )
                    .is_some();
                if added {
                    crate::widget::runtime::request_repaint(widget_id);
                }
                return added;
            }
            #[cfg(alloc_frugal)]
            {
                let _ = (widget_id, text);
                false
            }
        }

        /// Removes every item from a combo box.
        fn combo_box_clear_items(&self, widget_id: ObjectId) -> bool {
            #[cfg(not(alloc_frugal))]
            {
                let cleared = with_typed_widget::<
                    crate::widget::input_widgets::combobox::ComboBox,
                    _,
                >(widget_id, |combo| combo.clear())
                .is_some();
                if cleared {
                    crate::widget::runtime::request_repaint(widget_id);
                }
                return cleared;
            }
            #[cfg(alloc_frugal)]
            {
                let _ = widget_id;
                false
            }
        }

        /// Appends an item to a list box, repainting it.
        fn list_box_add_item(&self, widget_id: ObjectId, text: &str) -> bool {
            #[cfg(not(alloc_frugal))]
            {
                let added = with_typed_widget::<crate::widget::input_widgets::listbox::ListBox, _>(
                    widget_id,
                    |list| list.add_item(text.to_string()),
                )
                .is_some();
                if added {
                    crate::widget::runtime::request_repaint(widget_id);
                }
                return added;
            }
            #[cfg(alloc_frugal)]
            {
                let _ = (widget_id, text);
                false
            }
        }

        /// Removes one item from a list box by index.
        fn list_box_remove_item(&self, widget_id: ObjectId, index: usize) -> bool {
            #[cfg(not(alloc_frugal))]
            {
                let removed =
                    with_typed_widget::<crate::widget::input_widgets::listbox::ListBox, _>(
                        widget_id,
                        |list| {
                            if index < list.count() {
                                list.remove_item(index);
                                true
                            } else {
                                false
                            }
                        },
                    )
                    .unwrap_or(false);
                if removed {
                    crate::widget::runtime::request_repaint(widget_id);
                }
                return removed;
            }
            #[cfg(alloc_frugal)]
            {
                let _ = (widget_id, index);
                false
            }
        }

        /// Removes every item from a list box.
        fn list_box_clear_items(&self, widget_id: ObjectId) -> bool {
            #[cfg(not(alloc_frugal))]
            {
                let cleared =
                    with_typed_widget::<crate::widget::input_widgets::listbox::ListBox, _>(
                        widget_id,
                        |list| list.clear(),
                    )
                    .is_some();
                if cleared {
                    crate::widget::runtime::request_repaint(widget_id);
                }
                return cleared;
            }
            #[cfg(alloc_frugal)]
            {
                let _ = widget_id;
                false
            }
        }

        /// Reads one list-box item's text, straight off the control.
        fn list_box_item_text(&self, widget_id: ObjectId, index: usize) -> Option<String> {
            #[cfg(not(alloc_frugal))]
            {
                return with_typed_widget::<crate::widget::input_widgets::listbox::ListBox, _>(
                    widget_id,
                    |list| list.items().get(index).cloned(),
                )
                .flatten();
            }
            #[cfg(alloc_frugal)]
            {
                let _ = (widget_id, index);
                None
            }
        }

        /// Reads one combo-box item's text, straight off the control.
        fn combo_box_item_text(&self, widget_id: ObjectId, index: usize) -> Option<String> {
            #[cfg(not(alloc_frugal))]
            {
                return with_typed_widget::<crate::widget::input_widgets::combobox::ComboBox, _>(
                    widget_id,
                    |combo| combo.items().get(index).cloned(),
                )
                .flatten();
            }
            #[cfg(alloc_frugal)]
            {
                let _ = (widget_id, index);
                None
            }
        }

        fn set_widget_geometry(
            &self,
            widget_id: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) {
            #[cfg(not(alloc_frugal))]
            {
                if crate::widget::runtime::set_geometry(
                    widget_id,
                    crate::core::Rect::new(x, y, width, height),
                ) {
                    crate::widget::runtime::request_repaint(widget_id);
                }
            }
            #[cfg(alloc_frugal)]
            let _ = (widget_id, x, y, width, height);
        }

        /// Marks whether the mounted widget accepts IME input.
        ///
        /// IME enablement is not a widget property (it describes how the host
        /// routes composition events to the control), so it is kept in the
        /// backend's own state rather than on the widget.
        fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
            #[cfg(not(alloc_frugal))]
            if !crate::widget::runtime::is_mounted(widget_id) {
                return false;
            }
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .ime_enabled
                .insert(widget_id, enabled);
            true
        }

        fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .ime_enabled
                .get(&widget_id)
                .copied()
                .unwrap_or(false)
        }

        fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
            #[cfg(not(alloc_frugal))]
            if !crate::widget::runtime::is_mounted(widget_id) {
                return false;
            }
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .accessibility_names
                .insert(widget_id, name.to_string());
            true
        }

        /// Reads the control's accessible name.
        ///
        /// An explicit override wins; otherwise the control's own name is derived
        /// from its label and kind, which is what the widget trait documents. The
        /// old accessor returned an empty string unless a host had set one, so a
        /// control with a perfectly good label reported no accessible name at all.
        fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
            let override_name = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .accessibility_names
                .get(&widget_id)
                .cloned();
            if let Some(name) = override_name {
                return name;
            }
            #[cfg(not(alloc_frugal))]
            {
                return crate::widget::runtime::with_widget(widget_id, |widget| {
                    widget.accessible_name()
                })
                .unwrap_or_default();
            }
            #[cfg(alloc_frugal)]
            {
                let _ = widget_id;
                String::new()
            }
        }
    };
}
