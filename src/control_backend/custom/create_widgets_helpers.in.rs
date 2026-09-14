// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

macro_rules! impl_helpers {
    () => {
        fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .widget_trigger_queue
                .pop_front()
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
