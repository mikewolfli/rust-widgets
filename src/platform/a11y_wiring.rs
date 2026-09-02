//! A11y wiring: connects `FocusManager` to the platform's `AccessibilityBridge`.

#[cfg(not(feature = "mini"))]
use crate::event::focus::FocusManager;

/// Wire a `FocusManager` to the platform's `AccessibilityBridge` if available.
///
/// When the platform has an accessibility bridge, this connects focus
/// changes to `notify_focus_changed` so screen readers can track focus.
/// This is a no-op when no bridge is available.
#[cfg(not(feature = "mini"))]
pub fn wire_focus_manager_to_a11y(fm: &mut FocusManager) {
    let platform = crate::platform::runtime::get_platform();
    if let Some(bridge) = platform.accessibility_bridge() {
        // SAFETY: The bridge reference is guaranteed to outlive the FocusManager
        // because both are owned by the application lifecycle which outlives
        // any widget tree. The raw pointer is only used within the callback
        // which fires synchronously during FocusManager operations.
        let bridge_ptr: *const dyn crate::platform::accessibility::AccessibilityBridge =
            bridge as *const dyn crate::platform::accessibility::AccessibilityBridge;
        fm.set_a11y_callback(Box::new(move |id| {
            let bridge = unsafe { &*bridge_ptr };
            bridge.notify_focus_changed(id);
        }));
    }
}

#[cfg(all(test, not(feature = "mini")))]
mod tests {
    use crate::event::focus::FocusManager;

    #[test]
    fn wire_focus_manager_to_a11y_no_panic_when_no_bridge() {
        // Verifies that wiring doesn't panic when no platform is initialized
        // (no bridge available -> should be a no-op).
        let mut fm = FocusManager::new();
        super::wire_focus_manager_to_a11y(&mut fm);
        // No assertion needed — the function should not panic
        assert!(fm.focused_widget().is_none());
    }
}
