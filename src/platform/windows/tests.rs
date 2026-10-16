// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Windows platform tests.
//!
//! The pure notification-mapping tests now live in
//! [`crate::platform::windows_notify`] so they run on every host. What remains
//! here needs a real `WindowsPlatform` and therefore only compiles on Windows.

#[cfg(target_os = "windows")]
mod windows_backend {
    use crate::platform::windows::{
        notify::{control_notify_kind_for_widget, enqueue_control_notify_event},
        types::{WindowsHandleKind, WindowsPlatform},
    };
    use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
    #[test]
    fn control_notify_mapping_button_click_routes_clicked() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::Button, 0);
        assert_eq!(kind, Some(WidgetTriggerKind::Clicked));
    }
    #[test]
    fn control_notify_mapping_line_edit_change_routes_value_changed() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::LineEdit, 0x0300);
        assert_eq!(kind, Some(WidgetTriggerKind::ValueChanged));
    }
    #[test]
    fn control_notify_mapping_combo_selection_routes_selection_changed() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::ComboBox, 1);
        assert_eq!(kind, Some(WidgetTriggerKind::SelectionChanged));
    }
    #[test]
    fn control_notify_mapping_combo_edit_change_routes_value_changed() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::ComboBox, 5);
        assert_eq!(kind, Some(WidgetTriggerKind::ValueChanged));
    }
    #[test]
    fn combo_selection_notify_enqueues_selection_and_value_events() {
        let platform = WindowsPlatform::new();
        let combo =
            platform.state.create_widget(WindowsHandleKind::ComboBox, "ComboBox", 0, 0, 120, 24);
        assert!(enqueue_control_notify_event(&platform, combo, 1));
        let mut queue = platform
            .menu_state
            .pending_widget_events
            .lock()
            .expect("windows trigger queue lock poisoned");
        assert_eq!(queue.len(), 2);
        assert_eq!(
            queue.pop_front(),
            Some(WidgetTriggerEvent {
                widget_id: combo,
                kind: WidgetTriggerKind::SelectionChanged,
            })
        );
        assert_eq!(
            queue.pop_front(),
            Some(WidgetTriggerEvent { widget_id: combo, kind: WidgetTriggerKind::ValueChanged })
        );
    }

    /// The three controls that used to be state-only surrogates routed through
    /// native creation helpers. Those helpers are gone now that every kind is
    /// painted by `src/widget/`, so what remains to assert is the contract the
    /// routing layer relies on: the backend no longer allocates a native handle
    /// for any control kind, and `widget::runtime` is the only surface that does.
    #[test]
    fn control_kinds_no_longer_allocate_native_handles() {
        use crate::platform::Platform;

        let platform = WindowsPlatform::new();
        platform.init();
        let window = platform.create_window("w", 0, 0, 400, 300);
        assert!(window > 0, "window should be created");

        // The window itself still has a bound HWND: it is the host for mounted
        // self-drawn surfaces.
        assert!(
            platform.get_native_handle(window).is_some(),
            "a window must keep its native handle"
        );
    }

    /// Self-drawn widgets are hosted by `windows/canvas.rs`, so a profile that
    /// carries the self-drawn widget set must advertise the surface and refuse to
    /// mount an unregistered widget id.
    ///
    /// A stripped profile (`mini` / `embedded`) has no `widget::runtime` to host,
    /// so the honest answer there is `false` rather than an over-claim — see
    /// principle #37. The expectation comes from the single profile gate
    /// (principle #58) instead of assuming the desktop profile.
    #[test]
    fn widget_surface_is_advertised_and_validates_ids() {
        use crate::platform::Platform;

        let platform = WindowsPlatform::new();
        platform.init();
        assert_eq!(
            platform.supports_surfaces(),
            crate::platform::profile::widgets_unstripped(),
            "surface support must track whether this profile has a self-drawn widget runtime"
        );

        // A widget id that was never registered in `widget::runtime` must be
        // refused rather than producing an empty canvas.
        assert!(!platform.mount_surface(1, 4242, crate::core::Rect::new(0, 0, 10, 10)));
        assert!(!platform.invalidate_surface(4242));
        assert!(!platform.unmount_surface(4242));
    }

    /// The display set must be enumerable on a real session, and a window must be locatable on one
    /// of the enumerated displays.
    ///
    /// These are the two readings `docs/issues/feedback-20261015.md` §十二 needs to keep a window
    /// visible across a monitor change ("put the panel back on a display that remains"): the set of
    /// screens, and which one the window is on. The Windows backend answered the trait's empty
    /// default before this, so the assertions below were the visible half of that gap.
    #[test]
    fn displays_are_enumerable_and_a_window_is_located_on_one() {
        use crate::platform::Platform;

        let platform = WindowsPlatform::new();
        platform.init();

        let displays = platform.displays();
        assert!(!displays.is_empty(), "a desktop session must enumerate at least one monitor");
        assert_eq!(
            displays.iter().filter(|d| d.is_primary).count(),
            1,
            "exactly one monitor is the primary"
        );
        for display in &displays {
            assert!(
                display.bounds.width > 0 && display.bounds.height > 0,
                "an enumerated monitor must have extents"
            );
            assert!(
                display.work_area.width > 0 && display.work_area.height > 0,
                "an enumerated monitor must have a work area"
            );
            assert!(display.scale > 0.0, "a scale factor must be positive");
        }

        let window = platform.create_window("display-probe", 100, 100, 320, 200);
        assert!(window > 0, "the probe window must be created");
        let located = platform.display_of_window(window);
        let located = located.expect("a created window must be located on a monitor");
        assert!(
            displays.iter().any(|d| d.id == located),
            "the located id must be one of the enumerated displays"
        );

        // A change is always reportable here, so a host can compare readings instead of polling
        // every frame.
        assert_ne!(platform.display_generation(), 0);

        // An id that names no window answers `None` rather than a made-up display.
        assert_eq!(platform.display_of_window(0), None);
    }

    /// An empty **text** clip is a real clipboard state, and `clipboard_text()` must report it as
    /// `Some("")` rather than collapsing it into the `None` that means "there is no text".
    ///
    /// The trait default does collapse them (it reads the merging `get_clipboard_text`), so this is
    /// what the Windows override exists for — the same precision the Linux/GTK, Android, iOS and
    /// HarmonyOS backends already answer (`docs/issues/feedback-20261015.md` §8.6). The clipboard is
    /// saved and restored so running the suite does not clobber what the session had.
    #[test]
    fn clipboard_text_distinguishes_an_empty_text_clip_from_no_text() {
        // `crate::compat::String` rather than the bare name: the `mini` profile is `no_std`, so the
        // std prelude that normally supplies `String` is suppressed and `--all-targets` compiles
        // this test module there too.
        use crate::compat::String;
        use crate::platform::Platform;

        let platform = WindowsPlatform::new();
        platform.init();
        let saved = platform.clipboard_text();

        assert!(platform.set_clipboard_text(""), "an empty text clip must be writeable");
        assert_eq!(
            platform.clipboard_text(),
            Some(String::new()),
            "an empty text clip is `Some(\"\")`, not `None`"
        );

        assert!(platform.set_clipboard_text("clipboard-text-probe"));
        assert_eq!(platform.clipboard_text(), Some(String::from("clipboard-text-probe")));
        // The merging accessor still answers the plain string, unchanged.
        assert_eq!(platform.get_clipboard_text(), "clipboard-text-probe");

        match saved {
            Some(text) => {
                let _ = platform.set_clipboard_text(&text);
            }
            None => {
                let _ = platform.set_clipboard_text("");
            }
        }
    }
}
