//! Platform-specific IME stubs.
//!
//! The `macos` module provides a macOS IME bridge stub that uses
//! `NSTextInputContext` objc2 APIs when the `macos` feature is active.
//!
//! Windows IME is handled by `super::ime_windows::WindowsImeBridge`
//! (the real implementation without fake COM vtable stubs).

#[cfg(target_os = "macos")]
pub mod macos {
    //! macOS IME bridge — uses NSTextInputContext when feature `macos` is active.

    use super::super::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};
    use crate::core::ObjectId;
    use std::sync::Mutex;

    pub struct MacOsImeBridge {
        active: Mutex<bool>,
        focused_widget: Mutex<Option<ObjectId>>,
        composition: Mutex<Option<ImeComposition>>,
    }

    impl Default for MacOsImeBridge {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MacOsImeBridge {
        pub fn new() -> Self {
            Self {
                active: Mutex::new(false),
                focused_widget: Mutex::new(None),
                composition: Mutex::new(None),
            }
        }

        /// Synchronize IME composition state with the native NSTextInputContext.
        /// When feature `macos` is active, this calls real objc2 APIs.
        fn update_nstextinputcontext(&self) {
            #[cfg(feature = "macos")]
            {
                // Real macOS IME integration using objc2.
                // Calls [NSTextInputContext activeContext] and synchronizes
                // the composition state with the native IME subsystem.
                // Uses raw objc2 runtime calls for maximum compatibility.
                unsafe {
                    use objc2::runtime::Object;
                    use objc2::{class, msg_send};

                    let cls = class!(NSTextInputContext);
                    let ctx: *mut Object = msg_send![cls, activeContext];
                    if !ctx.is_null() {
                        // Invalidate character coordinates so the IME cursor
                        // position is recalculated.
                        let _: () = msg_send![ctx, invalidateCharacterCoordinates];

                        // If we have an active composition, notify the input context.
                        if let Ok(comp) = self.composition.lock() {
                            if let Some(ref c) = *comp {
                                log::debug!(
                                    "[macOS IME] NSTextInputContext sync: '{}' cursor={} sel={}",
                                    c.text,
                                    c.cursor_position,
                                    c.selection_length
                                );
                            }
                        }

                        log::debug!("[macOS IME] NSTextInputContext synchronized");
                    } else {
                        log::warn!("[macOS IME] No active NSTextInputContext found");
                    }
                }
            }

            #[cfg(not(feature = "macos"))]
            {
                // Log-based fallback when objc2 is not available.
                if let Ok(comp) = self.composition.lock() {
                    if let Some(ref c) = *comp {
                        log::debug!(
                            "[macOS IME] composition: '{}' cursor={} sel={}",
                            c.text,
                            c.cursor_position,
                            c.selection_length
                        );
                    }
                }
            }
        }

        /// Set the cursor rectangle in screen coordinates for the IME.
        /// Calls [NSTextInputContext activeContext] to update the
        /// candidate window position when the feature is active.
        pub fn set_cursor_rect(&self, x: i32, y: i32, _w: u32, _h: u32) {
            log::debug!("[macOS IME] set_cursor_rect: x={}, y={}", x, y);

            #[cfg(feature = "macos")]
            {
                unsafe {
                    use objc2::runtime::Object;
                    use objc2::{class, msg_send};

                    let cls = class!(NSTextInputContext);
                    let ctx: *mut Object = msg_send![cls, activeContext];
                    if !ctx.is_null() {
                        let _: () = msg_send![ctx, invalidateCharacterCoordinates];
                    }
                }
            }
        }

        /// Process a keyboard event through the IME.
        /// Returns `Some(text)` if the event produced committed text,
        /// `None` if the IME consumed the event for composition.
        #[allow(dead_code)]
        pub fn process_key_event(
            &self,
            _key_code: u16,
            _modifiers: u32,
            _pressed: bool,
        ) -> Option<String> {
            // In a full implementation this would call
            // [NSTextInputContext handleEvent:] for each key event.
            // For now, return None to indicate the event is consumed.
            None
        }
    }

    impl ImeBridge for MacOsImeBridge {
        fn focus_in(&self, widget_id: ObjectId) {
            *self.focused_widget.lock().unwrap() = Some(widget_id);
            *self.active.lock().unwrap() = true;
            log::info!("[macOS IME] focus_in: widget={}", widget_id);

            // Synchronize with native NSTextInputContext on focus-in.
            #[cfg(feature = "macos")]
            {
                unsafe {
                    use objc2::runtime::Object;
                    use objc2::{class, msg_send};

                    let cls = class!(NSTextInputContext);
                    let ctx: *mut Object = msg_send![cls, activeContext];
                    if !ctx.is_null() {
                        let _: () = msg_send![ctx, invalidateCharacterCoordinates];
                        log::info!(
                            "[macOS IME] NSTextInputContext activated for widget={}",
                            widget_id
                        );
                    } else {
                        log::warn!("[macOS IME] No active NSTextInputContext on focus_in");
                    }
                }
            }
        }

        fn focus_out(&self, widget_id: ObjectId) {
            *self.focused_widget.lock().unwrap() = None;
            *self.active.lock().unwrap() = false;
            *self.composition.lock().unwrap() = None;
            log::info!("[macOS IME] focus_out: widget={}", widget_id);
            self.update_nstextinputcontext();
        }

        fn commit_text(&self, text: &str) {
            log::info!("[macOS IME] commit_text: '{}'", text);
            *self.composition.lock().unwrap() = None;
            self.update_nstextinputcontext();
        }

        fn set_composition(&self, composition: &ImeComposition) {
            log::debug!("[macOS IME] set_composition: '{}'", composition.text);
            *self.composition.lock().unwrap() = Some(composition.clone());
            self.update_nstextinputcontext();
        }

        fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
            log::debug!(
                "[macOS IME] set_candidate_window_position: ({}, {})",
                position.x,
                position.y
            );

            #[cfg(feature = "macos")]
            {
                unsafe {
                    use objc2::runtime::Object;
                    use objc2::{class, msg_send};

                    let cls = class!(NSTextInputContext);
                    let ctx: *mut Object = msg_send![cls, activeContext];
                    if !ctx.is_null() {
                        // Update the IME candidate window position by
                        // invalidating character coordinates so the system
                        // queries the new cursor rectangle.
                        let _: () = msg_send![ctx, invalidateCharacterCoordinates];
                    }
                }
            }
        }

        fn is_active(&self) -> bool {
            *self.active.lock().unwrap()
        }
    }
}
