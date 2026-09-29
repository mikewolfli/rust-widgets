// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A11y wiring: connects `FocusManager` to the platform's `AccessibilityBridge`.

#[cfg(not(alloc_frugal))]
use crate::event::focus::FocusManager;

/// Wire a `FocusManager` to the platform's `AccessibilityBridge` if available.
///
/// When the platform has an accessibility bridge, this connects focus
/// changes to `notify_focus_changed` so screen readers can track focus.
/// This is a no-op when no bridge is available.
#[cfg(not(alloc_frugal))]
pub fn wire_focus_manager_to_a11y(fm: &mut FocusManager) {
    let platform = crate::platform::runtime::get_platform();
    if let Some(bridge) = platform.accessibility_bridge() {
        // `get_platform()` returns `&'static dyn Platform` (it is a `OnceLock`, and the test
        // override is also `&'static`), so the bridge reference already has the `'static` lifetime
        // the callback needs. Capturing it directly is sound and needs no `unsafe`; the previous
        // version erased it to a `*const dyn AccessibilityBridge` and re-dereferenced it inside the
        // closure, asserting a lifetime relationship the type system could not enforce.
        let bridge: &'static dyn crate::platform::accessibility::AccessibilityBridge = bridge;
        fm.set_a11y_callback(Box::new(move |id| {
            bridge.notify_focus_changed(id);
        }));
    }
}

#[cfg(all(test, not(alloc_frugal)))]
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
