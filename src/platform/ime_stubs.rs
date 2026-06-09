//! Platform-specific IME stubs.
//! These are placeholder implementations that will be replaced with
//! real platform IME bindings (NSTextInputContext, TSF, etc.).

#[cfg(target_os = "macos")]
pub mod macos {
    //! macOS IME stub — will use NSTextInputContext.
    //! Reference: NSTextInputClient protocol, NSInputManager, NSTextInputContext

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

        fn update_nstextinputcontext(&self) {
            // Called to synchronize NSTextInputContext state.
            // In a full implementation, this would call [NSTextInputContext activeContext]
            // and [NSTextInputContext handleEvent:] for key events.
            // For now, log the composition state for debugging.
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

    impl ImeBridge for MacOsImeBridge {
        fn focus_in(&self, widget_id: ObjectId) {
            *self.focused_widget.lock().unwrap() = Some(widget_id);
            *self.active.lock().unwrap() = true;
            log::info!("[macOS IME] focus_in: widget={}", widget_id);
        }

        fn focus_out(&self, widget_id: ObjectId) {
            *self.focused_widget.lock().unwrap() = None;
            *self.active.lock().unwrap() = false;
            *self.composition.lock().unwrap() = None;
            log::info!("[macOS IME] focus_out: widget={}", widget_id);
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
        }

        fn is_active(&self) -> bool {
            *self.active.lock().unwrap()
        }
    }
}

#[cfg(target_os = "windows")]
pub mod windows {
    //! Windows IME stub — will use TSF (Text Services Framework).
    //! Reference: ITfThreadMgr, ITfDocumentMgr, ITfContext

    use super::super::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};
    use crate::core::ObjectId;

    pub struct WindowsImeBridge;

    impl ImeBridge for WindowsImeBridge {
        fn focus_in(&self, _widget_id: ObjectId) {
            log::info!("[Windows IME] focus_in: placeholder");
        }
        fn focus_out(&self, _widget_id: ObjectId) {
            log::info!("[Windows IME] focus_out: placeholder");
        }
        fn commit_text(&self, _text: &str) {
            log::info!("[Windows IME] commit_text: placeholder");
        }
        fn set_composition(&self, _composition: &ImeComposition) {
            log::info!("[Windows IME] set_composition: placeholder");
        }
        fn set_candidate_window_position(&self, _position: ImeCandidatePosition) {
            log::info!("[Windows IME] set_candidate_window_position: placeholder");
        }
        fn is_active(&self) -> bool {
            false
        }
    }
}
