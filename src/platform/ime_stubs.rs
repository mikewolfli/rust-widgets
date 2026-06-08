//! Platform-specific IME stubs.
//! These are placeholder implementations that will be replaced with
//! real platform IME bindings (NSTextInputContext, TSF, etc.).

#[cfg(target_os = "macos")]
pub mod macos {
    //! macOS IME stub — will use NSTextInputContext.
    //! Reference: NSTextInputClient protocol, NSInputManager, NSTextInputContext

    use super::super::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};
    use crate::core::ObjectId;

    pub struct MacOsImeBridge;

    impl ImeBridge for MacOsImeBridge {
        fn focus_in(&self, _widget_id: ObjectId) {
            log::info!("[macOS IME] focus_in: placeholder");
        }
        fn focus_out(&self, _widget_id: ObjectId) {
            log::info!("[macOS IME] focus_out: placeholder");
        }
        fn commit_text(&self, _text: &str) {
            log::info!("[macOS IME] commit_text: placeholder");
        }
        fn set_composition(&self, _composition: &ImeComposition) {
            log::info!("[macOS IME] set_composition: placeholder");
        }
        fn set_candidate_window_position(&self, _position: ImeCandidatePosition) {
            log::info!("[macOS IME] set_candidate_window_position: placeholder");
        }
        fn is_active(&self) -> bool {
            false
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
