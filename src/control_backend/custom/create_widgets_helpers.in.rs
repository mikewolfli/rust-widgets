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
            lock(&self.state).widget_trigger_queue.pop_front()
        }

        /// Pops the next triggered widget id.
        ///
        /// Shares one queue with `poll_widget_trigger_event` — they are two views
        /// of the same event stream — so both consume an event when they answer,
        /// exactly as the platform backends do.
        fn poll_widget_triggered(&self) -> Option<ObjectId> {
            lock(&self.state).widget_trigger_queue.pop_front().map(|event| event.widget_id)
        }
        fn inject_widget_trigger_event(
            &self,
            widget_id: ObjectId,
            kind: WidgetTriggerKind,
        ) -> bool {
            let mut state = lock(&self.state);
            state.widget_trigger_queue.push_back(WidgetTriggerEvent { widget_id, kind });
            true
        }

        /// Reports a container's new client size and queues a `Resized` trigger.
        ///
        /// Only an id this backend actually created is accepted: the state map is the
        /// authority on what exists, so a stale id cannot inject a phantom resize that
        /// would then re-run a layout for a window that is gone.
        ///
        /// # Why a pending resize is not queued twice
        ///
        /// The event carries no size: it names a window, and the reader asks
        /// [`Self::window_client_size`] for the current one. So when a resize for a window
        /// is already pending, queuing another adds **no information** — whoever reads the
        /// first one will read the size the second one would have reported.
        ///
        /// This matters because a drag delivers a resize per allocation, and several
        /// allocations happen per drag step. Measured on the finance demo, a 20-step drag
        /// produced 263 layout runs; each one re-placed 7 controls and re-painted 7
        /// surfaces, and all of it ran on the GTK main thread — which is the thread that
        /// also has to deliver the next mouse event. The drag therefore lagged behind the
        /// pointer.
        ///
        /// Collapsing a run of pending resizes for one window to the one event that will
        /// report the final size keeps the observed behaviour (the layout runs for the size
        /// the window actually has) while removing the work that cannot be observed at all.
        fn queue_resize_trigger(&self, window_id: ObjectId, width: u32, height: u32) -> bool {
            // Only an id this backend actually created is accepted, so a stale id
            // cannot inject a phantom resize that would re-run a layout for a window
            // that is gone. A stripped profile keeps no widget objects in the runtime
            // (that is what `alloc_frugal`/`embedded_surface` mean), so the record map
            // itself — written below — is the authority there.
            #[cfg(not(alloc_frugal))]
            if !crate::widget::runtime::is_mounted(window_id) {
                return false;
            }
            let mut state = lock(&self.state);
            // The size is always recorded: it is what the pending event will be read
            // against, so the latest value must win even when no new event is queued.
            state.window_client_sizes.insert(window_id, (width, height));
            let already_pending = state.widget_trigger_queue.iter().any(|event| {
                event.widget_id == window_id && event.kind == WidgetTriggerKind::Resized
            });
            if !already_pending {
                state.widget_trigger_queue.push_back(WidgetTriggerEvent {
                    widget_id: window_id,
                    kind: WidgetTriggerKind::Resized,
                });
            }
            true
        }

        /// The client size last reported for `window_id`.
        ///
        /// Falls back to the widget's current geometry, which is what a window that has
        /// never been resized by the user still has. A stripped profile has no widget
        /// runtime to ask, so only an explicitly reported size answers there.
        fn window_client_size(&self, window_id: ObjectId) -> Option<(u32, u32)> {
            if let Some(size) = lock(&self.state).window_client_sizes.get(&window_id).copied() {
                return Some(size);
            }
            #[cfg(not(alloc_frugal))]
            return crate::widget::runtime::geometry_of(window_id)
                .map(|rect| (rect.width, rect.height));
            #[cfg(alloc_frugal)]
            {
                let _ = window_id;
                None
            }
        }

        /// Writes the control's label, repainting it.
        ///
        /// Kinds disagree on the property's name — a `Button` exposes `text`, a
        /// `Window`/`GroupBox` exposes `title`, a `StatusBar` exposes `message`, and a
        /// `FloatingLabel` exposes `label` for the caption it floats. Probing the known
        /// spellings in `LABEL_PROPERTY_NAMES` order keeps this accessor total without a
        /// `match` on kind, which would be the kind of central table this refactor
        /// removes. The first spelling is `label`, so a control that has a dedicated
        /// caption property is written through it rather than through its content.
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
        ///
        /// Mirrors [`Self::set_widget_text`], including its order: reading the same
        /// control back must answer with the property it was written through, or a
        /// caller would set a caption and read a different string.
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
                // Forward to the platform, which is the half that owns a real window.
                //
                // The widget's visibility is a model flag, and for an ordinary control
                // that flag is the whole story — the library paints it, so there is no
                // platform object to tell. It is **not** the whole story for a **window**:
                // a toplevel is a real OS object the platform created, and setting a flag
                // on the model cannot make it appear. Without this forward, `win.show()`
                // set a flag and the window never showed.
                //
                // This is why the defect was easy to miss: the call looked complete, and a
                // demo that happened to mount a surface onto its window appeared anyway
                // (the mount used to call `show_all` as a side effect). A demo that mounted
                // nothing never appeared at all, and its log still said the window was
                // shown.
                crate::platform::get_platform().set_widget_visible(widget_id, visible);
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
                let rect = crate::core::Rect::new(x, y, width, height);
                // Only a real change is worth acting on.
                //
                // A window that is being dragged delivers the same size many times: a
                // resize handler runs per allocation, several allocations happen per drag
                // step, and a layout is re-run for each. Measured on the finance demo, 62% of
                // its 1728 layout runs repeated a size that had already been applied (one
                // size arrived 298 times). Acting on every one of those re-painted every
                // mounted control for an identical rectangle — which is invisible work that
                // shows up as a stutter, because the paint cost is paid per frame rather than
                // per change.
                //
                // An unchanged rectangle is therefore skipped: the widget is already there,
                // and a repaint of identical pixels cannot change what is on screen.
                let unchanged = crate::widget::runtime::geometry_of(widget_id)
                    .map(|current| current == rect)
                    .unwrap_or(false);
                if unchanged {
                    return;
                }
                if crate::widget::runtime::set_geometry(widget_id, rect) {
                    crate::widget::runtime::request_repaint(widget_id);
                    // Tell the platform the surface moved, not just the model.
                    //
                    // A mounted widget lives in two places: the registry (its geometry,
                    // which the layout writes and the painter reads) and the native surface
                    // the backend allocated for it. Without this second call a window
                    // resize changed the geometry the *painter* used — the chart redrew at
                    // its new size — while the native container kept the allocation it was
                    // mounted with, so the control's contents changed inside a box that
                    // stayed the same size.
                    //
                    // A backend without mounted surfaces reports `false`, which is not an
                    // error: the model geometry is authoritative for everything the library
                    // paints itself.
                    crate::resize_surface(widget_id, rect);
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
            lock(&self.state).ime_enabled.insert(widget_id, enabled);
            true
        }

        fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
            lock(&self.state).ime_enabled.get(&widget_id).copied().unwrap_or(false)
        }

        fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
            #[cfg(not(alloc_frugal))]
            if !crate::widget::runtime::is_mounted(widget_id) {
                return false;
            }
            lock(&self.state).accessibility_names.insert(widget_id, name.to_string());
            true
        }

        /// Reads the control's accessible name.
        ///
        /// An explicit override wins; otherwise the control's own name is derived
        /// from its label and kind, which is what the widget trait documents. The
        /// old accessor returned an empty string unless a host had set one, so a
        /// control with a perfectly good label reported no accessible name at all.
        fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
            let override_name = lock(&self.state).accessibility_names.get(&widget_id).cloned();
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

        /// Derives a full accessibility state for a mounted control.
        ///
        /// # Why the mounted lookup is the whole implementation
        ///
        /// `A11yState::from_widget` asks the control for its own role, name, description, value
        /// and — the part that was missing — its `checked`/`mixed` state, all through the property
        /// contract. That means this accessor adds no mapping of its own: it is just the point at
        /// which a live control becomes announceable. An id that addresses nothing returns `None`
        /// rather than an all-defaults state, so a stale id cannot be mistaken for a real node.
        fn widget_a11y_state(
            &self,
            widget_id: ObjectId,
        ) -> Option<crate::platform::accessibility::A11yState> {
            #[cfg(not(alloc_frugal))]
            {
                return crate::widget::runtime::with_widget(widget_id, |widget| {
                    crate::platform::accessibility::A11yState::from_widget(widget)
                });
            }
            #[cfg(alloc_frugal)]
            {
                let _ = widget_id;
                None
            }
        }
    };
}
