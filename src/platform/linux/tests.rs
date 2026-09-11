//! Integration tests for the Linux (GTK) backend.
//!
//! These tests verify platform creation, basic widget lifecycle, dialog
//! creation, and clipboard roundtrip for the Linux platform backend.
//!
//! # GTK threading constraint
//!
//! GTK 3 allows `gtk::init()` to succeed on exactly one thread per process and
//! aborts the process if a second thread tries to initialize it. The Rust test
//! harness runs each `#[test]` on its own worker thread, so running one GTK test
//! per function would either fail or segfault. All GTK-backed scenarios are
//! therefore exercised from a single `#[test]` function so GTK is initialized
//! once; the state-only assertions remain in separate tests because they never
//! touch GTK.
//!
//! When no display is available `gtk::init()` fails, and the combined test
//! falls back to asserting the honest state-backend contract instead of
//! pretending the native GTK path ran.

use crate::platform::linux::LinuxPlatform;
use crate::platform::Platform;
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::platform::{DropEvent, WidgetTriggerKind};

/// Initialize GTK on the current thread.
///
/// Returns `false` when no display is reachable, which selects the state-only
/// fallback path in the combined GTK test.
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
fn ensure_gtk() -> bool {
    gtk::init().is_ok()
}

/// The Round-2 containers must be parent-validated on the state backend too, so
/// behaviour is identical whether or not GTK is compiled in.
///
/// Under `gtk-native` this runs as part of `gtk_native_backend_lifecycle`
/// instead: GTK permits only one `gtk::init()` per process, so a second test
/// that initialises GTK would abort with "Attempted to initialize GTK from two
/// different threads".
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
#[test]
fn round2_containers_require_valid_parent() {
    let backend = LinuxPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 400, 300);
    assert!(window > 0);

    let bogus = 4242;
    assert_eq!(backend.create_group_box(bogus, "g", 0, 0, 10, 10), 0);
    assert_eq!(backend.create_frame(bogus, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_tab_widget(bogus, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_splitter(bogus, 0, 0, 10, 10), 0);

    assert!(backend.create_group_box(window, "Group", 0, 0, 100, 60) > 0);
    assert!(backend.create_frame(window, 0, 70, 100, 60) > 0);
    assert!(backend.create_tab_widget(window, 0, 140, 100, 60) > 0);
    assert!(backend.create_splitter(window, 0, 210, 100, 60) > 0);
}

/// Round-3 controls must be parent-validated on the state backend (the
/// `gtk-native` variant of this coverage lives in `gtk_native_backend_lifecycle`,
/// because GTK allows only one `gtk::init()` per process).
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
#[test]
fn round3_controls_require_valid_parent() {
    let backend = LinuxPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 400, 300);
    assert!(window > 0);

    let bogus = 4242;
    assert_eq!(backend.create_toggle_button(bogus, "t", 0, 0, 10, 10), 0);
    assert_eq!(backend.create_calendar(bogus, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_scroll_bar(bogus, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_double_spin_box(bogus, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_font_combo_box(bogus, 0, 0, 10, 10), 0);

    assert!(backend.create_toggle_button(window, "T", 0, 0, 80, 24) > 0);
    assert!(backend.create_calendar(window, 0, 30, 160, 120) > 0);
    assert!(backend.create_scroll_bar(window, 0, 160, 16, 100) > 0);
    assert!(backend.create_double_spin_box(window, 0, 270, 80, 24) > 0);
    assert!(backend.create_font_combo_box(window, 0, 300, 120, 24) > 0);
}

/// Round-4 dialog/popup/context-menu kinds must be parent-validated on the
/// state backend (the `gtk-native` coverage lives in
/// `gtk_native_backend_lifecycle`, since GTK allows one `gtk::init()`).
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
#[test]
fn round4_dialogs_require_valid_parent() {
    let backend = LinuxPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 400, 300);
    assert!(window > 0);

    let bogus = 4242;
    assert_eq!(backend.create_context_menu(bogus, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_popup_window(bogus, "p", 0, 0, 10, 10), 0);
    assert_eq!(backend.create_dialog(bogus, "d", 0, 0, 10, 10), 0);
    assert_eq!(backend.create_input_dialog(bogus, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_progress_dialog(bogus, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_directory_dialog(bogus, "t", 0, 0, 10, 10), 0);

    assert!(backend.create_context_menu(window, 0, 0, 120, 80) > 0);
    assert!(backend.create_popup_window(window, "P", 0, 0, 160, 100) > 0);
    assert!(backend.create_dialog(window, "D", 0, 0, 240, 160) > 0);
    assert!(backend.create_input_dialog(window, 0, 0, 240, 160) > 0);
    assert!(backend.create_progress_dialog(window, 0, 0, 240, 160) > 0);
    assert!(backend.create_directory_dialog(window, "Dir", 0, 0, 240, 160) > 0);
}

/// Round-5 pickers / busy indicator must be parent-validated on the state
/// backend (the `gtk-native` coverage lives in `gtk_native_backend_lifecycle`).
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
#[test]
fn round5_pickers_require_valid_parent() {
    let backend = LinuxPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 400, 300);
    assert!(window > 0);

    let bogus = 4242;
    assert_eq!(backend.create_date_picker(bogus, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_time_picker(bogus, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_date_time_picker(bogus, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_activity_indicator(bogus, 0, 0, 10, 10), 0);

    assert!(backend.create_date_picker(window, 0, 0, 140, 30) > 0);
    assert!(backend.create_time_picker(window, 0, 40, 140, 30) > 0);
    assert!(backend.create_date_time_picker(window, 0, 80, 240, 30) > 0);
    assert!(backend.create_activity_indicator(window, 0, 120, 40, 40) > 0);
}

/// Exercise the full GTK-backed Linux backend from one thread.
///
/// Covers: window + control creation, text/show/hide/enable/geometry
/// roundtrips, menu lifecycle, native dialog creation, and clipboard.
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
#[test]
fn gtk_native_backend_lifecycle() {
    if !ensure_gtk() {
        // Honest fallback: without a display the native backend cannot run.
        let backend = LinuxPlatform::new();
        backend.init();
        assert_eq!(backend.backend_name(), "linux-state-backend");
        return;
    }

    let backend = LinuxPlatform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "gtk");

    // ── Window + control creation ──
    let window = backend.create_window("TestWindow", 50, 50, 400, 300);
    assert!(window > 0, "Window should be created");

    let button = backend.create_button(window, "Click", 10, 10, 80, 24);
    assert!(button > 0, "Button should be created");

    let label = backend.create_label(window, "Hello", 10, 40, 80, 24);
    assert!(label > 0, "Label should be created");

    let checkbox = backend.create_checkbox(window, "Check", 10, 70, 80, 24);
    assert!(checkbox > 0, "Checkbox should be created");

    let line_edit = backend.create_line_edit(window, "edit", 10, 100, 160, 24);
    assert!(line_edit > 0, "LineEdit should be created");

    let radio = backend.create_radio_button(window, "Radio", 10, 130, 80, 24);
    assert!(radio > 0, "RadioButton should be created");

    let slider = backend.create_slider(window, 10, 160, 200, 24);
    assert!(slider > 0, "Slider should be created");

    let progress = backend.create_progress_bar(window, 10, 190, 200, 24);
    assert!(progress > 0, "ProgressBar should be created");

    let combo = backend.create_combo_box(window, 10, 220, 140, 24);
    assert!(combo > 0, "ComboBox should be created");

    let list_box = backend.create_list_box(window, 10, 250, 140, 80);
    assert!(list_box > 0, "ListBox should be created");

    let spin = backend.create_spin_box(window, 10, 340, 80, 24);
    assert!(spin > 0, "SpinBox should be created");

    let list_view = backend.create_list_view(window, 10, 370, 140, 80);
    assert!(list_view > 0, "ListView should be created");

    let scroll = backend.create_scroll_area(window, 10, 460, 140, 80);
    assert!(scroll > 0, "ScrollArea should be created");

    // ── Round-2 native containers ──
    // These previously degraded to a plain panel on the native path; each must
    // now be the real GTK widget so it keeps its identity.
    let group_box = backend.create_group_box(window, "Group", 10, 560, 180, 80);
    assert!(group_box > 0, "GroupBox should be created");
    let frame = backend.create_frame(window, 10, 650, 180, 60);
    assert!(frame > 0, "Frame should be created");
    let tabs = backend.create_tab_widget(window, 10, 720, 180, 100);
    assert!(tabs > 0, "TabWidget should be created");
    let splitter = backend.create_splitter(window, 10, 830, 180, 100);
    assert!(splitter > 0, "Splitter should be created");

    // Parent validation must hold on the native path too (mirrors the
    // state-only test, which is skipped under gtk-native because GTK allows a
    // single `gtk::init()` per process).
    {
        let bogus = 4242;
        assert_eq!(backend.create_group_box(bogus, "g", 0, 0, 10, 10), 0);
        assert_eq!(backend.create_frame(bogus, 0, 0, 10, 10), 0);
        assert_eq!(backend.create_tab_widget(bogus, 0, 0, 10, 10), 0);
        assert_eq!(backend.create_splitter(bogus, 0, 0, 10, 10), 0);
    }

    {
        use crate::core::MutexExt;
        use gtk::prelude::*;
        let native = backend.native.lock_guard();

        let gb = native
            .widgets
            .get(&group_box)
            .and_then(|w| w.clone().downcast::<gtk::Frame>().ok())
            .expect("GroupBox must be a native GTK Frame");
        assert_eq!(gb.label().as_deref(), Some("Group"), "Frame must carry the group title");

        native
            .widgets
            .get(&frame)
            .and_then(|w| w.clone().downcast::<gtk::Frame>().ok())
            .expect("Frame must be a native GTK Frame");

        let nb = native
            .widgets
            .get(&tabs)
            .and_then(|w| w.clone().downcast::<gtk::Notebook>().ok())
            .expect("TabWidget must be a native GTK Notebook");
        assert!(nb.n_pages() >= 1, "Notebook must have at least one page");

        let paned = native
            .widgets
            .get(&splitter)
            .and_then(|w| w.clone().downcast::<gtk::Paned>().ok())
            .expect("Splitter must be a native GTK Paned");
        assert!(paned.position() > 0, "Paned divider must be positioned");
    }

    // ── Round-3 native controls ──
    let toggle = backend.create_toggle_button(window, "Toggle", 10, 940, 120, 30);
    assert!(toggle > 0, "ToggleButton should be created");
    let cal = backend.create_calendar(window, 10, 980, 200, 160);
    assert!(cal > 0, "Calendar should be created");
    let sbar = backend.create_scroll_bar(window, 10, 1150, 20, 120);
    assert!(sbar > 0, "ScrollBar should be created");
    let dspin = backend.create_double_spin_box(window, 10, 1280, 100, 30);
    assert!(dspin > 0, "DoubleSpinBox should be created");
    let fcombo = backend.create_font_combo_box(window, 10, 1320, 140, 30);
    assert!(fcombo > 0, "FontComboBox should be created");

    {
        use crate::core::MutexExt;
        use gtk::prelude::*;
        let native = backend.native.lock_guard();

        native
            .widgets
            .get(&toggle)
            .and_then(|w| w.clone().downcast::<gtk::ToggleButton>().ok())
            .expect("ToggleButton must be a native GTK ToggleButton");

        native
            .widgets
            .get(&cal)
            .and_then(|w| w.clone().downcast::<gtk::Calendar>().ok())
            .expect("Calendar must be a native GTK Calendar");

        native
            .widgets
            .get(&sbar)
            .and_then(|w| w.clone().downcast::<gtk::Scrollbar>().ok())
            .expect("ScrollBar must be a native GTK Scrollbar");

        let ds = native
            .widgets
            .get(&dspin)
            .and_then(|w| w.clone().downcast::<gtk::SpinButton>().ok())
            .expect("DoubleSpinBox must be a native GTK SpinButton");
        assert_eq!(ds.digits(), 2, "DoubleSpinBox must keep its fractional digits");

        let fc = native
            .widgets
            .get(&fcombo)
            .and_then(|w| w.clone().downcast::<gtk::ComboBoxText>().ok())
            .expect("FontComboBox must be a native GTK ComboBoxText");
        assert!(fc.active().is_some(), "FontComboBox must have a selected family");
    }

    // ── Round-4 dialogs / popups / context menu ──
    let ctx_menu = backend.create_context_menu(window, 0, 0, 120, 80);
    assert!(ctx_menu > 0, "ContextMenu should be created");
    let popup = backend.create_popup_window(window, "Popup", 0, 0, 160, 100);
    assert!(popup > 0, "PopupWindow should be created");
    let dlg = backend.create_dialog(window, "Dialog", 0, 0, 240, 160);
    assert!(dlg > 0, "Dialog should be created");
    let input_dlg = backend.create_input_dialog(window, 0, 0, 240, 160);
    assert!(input_dlg > 0, "InputDialog should be created");
    let prog_dlg = backend.create_progress_dialog(window, 0, 0, 240, 160);
    assert!(prog_dlg > 0, "ProgressDialog should be created");
    let dir_dlg = backend.create_directory_dialog(window, "Pick Folder", 0, 0, 240, 160);
    assert!(dir_dlg > 0, "DirectoryDialog should be created");

    {
        use crate::core::MutexExt;
        use gtk::prelude::*;
        let native = backend.native.lock_guard();

        // A context menu is a real GTK Menu registered for on-demand popup.
        assert!(native.menus.contains_key(&ctx_menu), "ContextMenu must be a real GTK Menu");
        native
            .widgets
            .get(&popup)
            .and_then(|w| w.clone().downcast::<gtk::Window>().ok())
            .expect("PopupWindow must be a native GTK Window");
        for (label, id) in
            [("Dialog", dlg), ("InputDialog", input_dlg), ("ProgressDialog", prog_dlg)]
        {
            native
                .widgets
                .get(&id)
                .and_then(|w| w.clone().downcast::<gtk::Dialog>().ok())
                .unwrap_or_else(|| panic!("{label} must be a native GTK Dialog"));
        }
        native
            .widgets
            .get(&dir_dlg)
            .and_then(|w| w.clone().downcast::<gtk::FileChooserDialog>().ok())
            .expect("DirectoryDialog must be a native GTK FileChooserDialog");
    }

    // ── Round-5 pickers / busy indicator ──
    let dp = backend.create_date_picker(window, 0, 0, 140, 30);
    assert!(dp > 0, "DatePicker should be created");
    let tp = backend.create_time_picker(window, 0, 0, 140, 30);
    assert!(tp > 0, "TimePicker should be created");
    let dtp = backend.create_date_time_picker(window, 0, 0, 240, 30);
    assert!(dtp > 0, "DateTimePicker should be created");
    let ai = backend.create_activity_indicator(window, 0, 0, 40, 40);
    assert!(ai > 0, "ActivityIndicator should be created");

    {
        use crate::core::MutexExt;
        use gtk::prelude::*;
        let native = backend.native.lock_guard();

        // DatePicker is composed around a real MenuButton + Calendar popover.
        native
            .widgets
            .get(&dp)
            .and_then(|w| w.clone().downcast::<gtk::MenuButton>().ok())
            .expect("DatePicker must be a native GTK MenuButton (calendar popover)");

        // TimePicker is composed from real SpinButtons.
        native
            .widgets
            .get(&tp)
            .and_then(|w| w.clone().downcast::<gtk::Box>().ok())
            .expect("TimePicker must be a native GTK Box of spin buttons");

        native
            .widgets
            .get(&dtp)
            .and_then(|w| w.clone().downcast::<gtk::Box>().ok())
            .expect("DateTimePicker must be a native GTK Box composite");

        // Busy indicator must be a real, running gtK::Spinner.
        native
            .widgets
            .get(&ai)
            .and_then(|w| w.clone().downcast::<gtk::Spinner>().ok())
            .expect("ActivityIndicator must be a native GTK Spinner");
    }

    // ── Widget lifecycle roundtrips ──
    backend.set_widget_text(button, "updated");
    assert_eq!(backend.get_widget_text(button), "updated");

    backend.show_widget(button);
    assert!(backend.is_widget_visible(button), "Button should be visible after show");
    backend.hide_widget(button);
    assert!(!backend.is_widget_visible(button), "Button should be hidden after hide");

    backend.set_widget_enabled(button, false);
    assert!(!backend.is_widget_enabled(button), "Button should be disabled");
    backend.set_widget_enabled(button, true);
    assert!(backend.is_widget_enabled(button), "Button should be enabled");

    backend.set_widget_geometry(button, 20, 20, 120, 32);

    // ── Text propagation to non-label GTK widgets ──
    // These previously fell through the downcast chain and silently did nothing.
    backend.set_widget_text(progress, "0.5");
    {
        use crate::core::MutexExt;
        use gtk::prelude::*;
        let native = backend.native.lock_guard();
        let progress_widget = native
            .widgets
            .get(&progress)
            .and_then(|w| w.clone().downcast::<gtk::ProgressBar>().ok())
            .expect("ProgressBar must be a native GTK ProgressBar");
        assert!(
            (progress_widget.fraction() - 0.5).abs() < f64::EPSILON,
            "ProgressBar fraction should follow numeric text"
        );
    }
    backend.set_widget_text(spin, "42");
    {
        use crate::core::MutexExt;
        use gtk::prelude::*;
        let native = backend.native.lock_guard();
        let spin_widget = native
            .widgets
            .get(&spin)
            .and_then(|w| w.clone().downcast::<gtk::SpinButton>().ok())
            .expect("SpinBox must be a native GTK SpinButton");
        assert!(
            (spin_widget.value() - 42.0).abs() < f64::EPSILON,
            "SpinButton value should follow numeric text"
        );
    }

    backend.set_widget_text(window, "UpdatedTitle");
    assert_eq!(backend.get_widget_text(window), "UpdatedTitle");
    backend.show_widget(window);
    assert!(backend.is_widget_visible(window), "Window should be visible");
    backend.hide_widget(window);
    assert!(!backend.is_widget_visible(window), "Window should be hidden");

    // ── Menu lifecycle ──
    let menu_bar = backend.create_menu_bar(window, 0, 0, 400, 24);
    assert!(menu_bar > 0, "MenuBar should be created");
    let menu = backend.create_menu(menu_bar, "File", 0, 0, 80, 24);
    assert!(menu > 0, "Menu should be created");
    let item = backend.menu_add_item(menu, "Open", Some("Ctrl+O"));
    assert!(item > 0, "MenuItem should be created");
    assert!(backend.attach_menu_bar_to_window(window, menu_bar));
    assert!(backend.inject_menu_trigger(item));
    assert_eq!(backend.poll_menu_triggered(), Some(item));

    // The menu bar must expose a real GTK MenuBar carrying the created items.
    {
        use crate::core::MutexExt;
        let native = backend.native.lock_guard();
        assert!(native.menu_bars.contains_key(&menu_bar), "MenuBar must be a native GTK MenuBar");
        assert!(native.menus.contains_key(&menu), "Menu must be a native GTK Menu");
    }

    // ── Native dialogs ──
    let msg = backend.create_message_box(window, "Title", "Body", 0, 0, 300, 150);
    assert!(msg > 0, "MessageBox should be created");
    let file = backend.create_file_dialog(window, 0, 0, 400, 300);
    assert!(file > 0, "FileDialog should be created");
    let color = backend.create_color_dialog(window, 0, 0, 300, 200);
    assert!(color > 0, "ColorDialog should be created");
    let font = backend.create_font_dialog(window, 0, 0, 300, 200);
    assert!(font > 0, "FontDialog should be created");

    // The dialogs must be registered as *native* GTK objects, not only as
    // logical state handles — otherwise the wiring is a no-op.
    {
        use crate::core::MutexExt;
        let native = backend.native.lock_guard();
        assert!(native.dialogs.contains_key(&msg), "MessageBox must have a native GTK dialog");
        assert!(native.dialogs.contains_key(&file), "FileDialog must have a native GTK dialog");
        assert!(native.dialogs.contains_key(&color), "ColorDialog must have a native GTK dialog");
        assert!(native.dialogs.contains_key(&font), "FontDialog must have a native GTK dialog");
        assert!(
            native.color_choosers.contains_key(&color),
            "ColorDialog must expose a ColorChooser"
        );
        assert!(native.font_choosers.contains_key(&font), "FontDialog must expose a FontChooser");
    }

    // ── Popup / typed triggers ──
    assert!(backend.inject_widget_trigger_event(button, WidgetTriggerKind::Clicked));
    let ev = backend.poll_widget_trigger_event().expect("trigger event");
    assert_eq!(ev.widget_id, button);
    assert_eq!(ev.kind, WidgetTriggerKind::Clicked);

    // ── Clipboard (native GTK/GDK path) ──
    assert!(backend.set_clipboard_text("linux_clip_test"));
    assert_eq!(backend.get_clipboard_text(), "linux_clip_test");
    assert!(backend.set_clipboard_text(""));
    assert_eq!(backend.get_clipboard_text(), "");

    // Prove the value reached the real GDK display clipboard and not only the
    // in-process mirror: read it back through an independent Clipboard handle.
    backend.set_clipboard_text("native-clipboard-proof");
    if let Some(display) = gtk::gdk::Display::default() {
        if let Some(clipboard) = gtk::Clipboard::default(&display) {
            let native = clipboard.wait_for_text().map(|s| s.to_string());
            assert_eq!(
                native.as_deref(),
                Some("native-clipboard-proof"),
                "clipboard text must be visible through the native GDK clipboard"
            );
        }
    }

    // ── Accessibility (AT-SPI bridge is authoritative) ──
    assert!(backend.set_widget_accessibility_name(button, "OK Button"));
    assert_eq!(backend.get_widget_accessibility_name(button), "OK Button");
    assert!(backend.set_widget_accessibility_name(button, "Updated Button"));
    assert_eq!(backend.get_widget_accessibility_name(button), "Updated Button");

    // The name must be visible through the platform a11y bridge, proving the
    // setter actually forwards to it rather than only recording logical state.
    if let Some(bridge) =
        <crate::platform::linux::LinuxPlatform as Platform>::accessibility_bridge(&backend)
    {
        assert_eq!(
            bridge.accessibility_name(button).as_deref(),
            Some("Updated Button"),
            "accessibility name must reach the AT-SPI bridge"
        );
    }

    // ── Drag and drop ──
    assert!(backend.begin_drag(button, "text/plain", b"payload"));
    assert!(backend.poll_drop_event().is_some());
    assert!(backend.inject_drop_event(DropEvent {
        source_widget_id: button,
        target_widget_id: button,
        mime: "text/plain".into(),
        payload: vec![],
    }));
}

/// State-only backend contract: no display and no GTK objects are involved.
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
#[test]
fn state_backend_lifecycle() {
    let backend = LinuxPlatform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "linux-state-backend");

    let window = backend.create_window("TestWindow", 50, 50, 400, 300);
    assert!(window > 0, "Window should be created");

    let button = backend.create_button(window, "Click", 10, 10, 80, 24);
    assert!(button > 0, "Button should be created");

    backend.set_widget_text(button, "updated");
    assert_eq!(backend.get_widget_text(button), "updated");

    backend.hide_widget(button);
    assert!(!backend.is_widget_visible(button), "Button should be hidden after hide");
    backend.show_widget(button);
    assert!(backend.is_widget_visible(button), "Button should be visible after show");

    backend.set_widget_enabled(button, false);
    assert!(!backend.is_widget_enabled(button), "Button should be disabled");

    // Accessibility forwarding works without GTK too: the AT-SPI bridge is a
    // Linux-only (not gtk-gated) component.
    assert!(backend.set_widget_accessibility_name(button, "OK Button"));
    assert_eq!(backend.get_widget_accessibility_name(button), "OK Button");
    if let Some(bridge) = <LinuxPlatform as Platform>::accessibility_bridge(&backend) {
        assert_eq!(
            bridge.accessibility_name(button).as_deref(),
            Some("OK Button"),
            "accessibility name must reach the AT-SPI bridge"
        );
    }
}
