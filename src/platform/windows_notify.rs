//! Pure Win32 notification-code semantics.
//!
//! The mappings in this module translate raw Win32 control notification codes
//! (`BN_CLICKED`, `CBN_SELCHANGE`, `EN_CHANGE`, ...) into the platform-neutral
//! [`WidgetTriggerKind`] used by the rest of the library. They contain no OS
//! calls and no handles, so unlike the rest of the Windows backend they are
//! compiled on every host. Keeping them here means they stay type-checked and
//! unit-tested on non-Windows CI instead of only being verified on a Windows
//! machine (the surrounding `src/platform/windows` module is
//! `#[cfg(target_os = "windows")]`-gated).

#[cfg(any(target_os = "windows", test))]
use crate::platform::WidgetTriggerKind;

/// Handle kinds tracked by the Windows backend.
///
/// This enum is pure data (no OS handles), so it lives in this ungated module
/// and is re-exported from `crate::platform::windows::types` to preserve the
/// existing public path.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WindowsHandleKind {
    Window,
    Button,
    Label,
    CheckBox,
    RadioButton,
    LineEdit,
    ListBox,
    Panel,
    MenuBar,
    Menu,
    MenuItem,
    ToolBar,
    StatusBar,
    ProgressBar,
    Slider,
    ComboBox,
    MessageBox,
    FileDialog,
    ColorDialog,
    FontDialog,
    SpinBox,
    ListView,
    ScrollArea,
    GroupBox,
    Frame,
    TabWidget,
    Splitter,
    ToggleButton,
    Calendar,
    ScrollBar,
    DoubleSpinBox,
    FontComboBox,
    ContextMenu,
    PopupWindow,
    Dialog,
    InputDialog,
    ProgressDialog,
    DirectoryDialog,
    DatePicker,
    TimePicker,
    DateTimePicker,
    ActivityIndicator,
}

/// `BN_CLICKED` — button/checkbox/radio button click notification.
#[cfg(any(target_os = "windows", test))]
pub(crate) const BN_CLICKED: u32 = 0;
/// `CBN_SELCHANGE` — combo box selection changed.
#[cfg(any(target_os = "windows", test))]
pub(crate) const CBN_SELCHANGE: u32 = 1;
/// `CBN_EDITCHANGE` — combo box edit field content changed.
#[cfg(any(target_os = "windows", test))]
pub(crate) const CBN_EDITCHANGE: u32 = 5;
/// `EN_CHANGE` — edit control content changed.
#[cfg(any(target_os = "windows", test))]
pub(crate) const EN_CHANGE: u32 = 0x0300;
/// `LBN_SELCHANGE` — list box selection changed.
#[cfg(any(target_os = "windows", test))]
pub(crate) const LBN_SELCHANGE: u32 = 1;

/// Map a Win32 notification code to a [`WidgetTriggerKind`] for the given widget
/// kind. Returns `None` when the code carries no meaning for that control.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn control_notify_kind_for_widget(
    kind: WindowsHandleKind,
    notify_code: u32,
) -> Option<WidgetTriggerKind> {
    match kind {
        WindowsHandleKind::Button
        | WindowsHandleKind::CheckBox
        | WindowsHandleKind::RadioButton => {
            if notify_code == BN_CLICKED {
                Some(WidgetTriggerKind::Clicked)
            } else {
                None
            }
        }
        WindowsHandleKind::LineEdit => {
            if notify_code == EN_CHANGE {
                Some(WidgetTriggerKind::ValueChanged)
            } else {
                None
            }
        }
        WindowsHandleKind::ComboBox => {
            if notify_code == CBN_SELCHANGE {
                Some(WidgetTriggerKind::SelectionChanged)
            } else if notify_code == CBN_EDITCHANGE {
                Some(WidgetTriggerKind::ValueChanged)
            } else {
                None
            }
        }
        WindowsHandleKind::ListBox => {
            if notify_code == LBN_SELCHANGE {
                Some(WidgetTriggerKind::SelectionChanged)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Whether a combo box notification should be dual-emitted as both a selection
/// change and a value change, for cross-platform event parity.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn combo_selchange_is_dual_emitted(
    kind: WindowsHandleKind,
    kind_trigger: WidgetTriggerKind,
) -> bool {
    kind == WindowsHandleKind::ComboBox && kind_trigger == WidgetTriggerKind::SelectionChanged
}

/// Resolve a [`WidgetTriggerKind`] from a widget kind and raw `WM_NOTIFY` code.
/// Used by the `WM_NOTIFY` handler in the window procedure.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn notify_kind_for_widget(
    kind: WindowsHandleKind,
    notify_code: u32,
) -> Option<WidgetTriggerKind> {
    if kind == WindowsHandleKind::Slider {
        return Some(WidgetTriggerKind::ValueChanged);
    }
    if notify_code == 0 {
        return None;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_click_routes_clicked() {
        assert_eq!(
            control_notify_kind_for_widget(WindowsHandleKind::Button, BN_CLICKED),
            Some(WidgetTriggerKind::Clicked)
        );
    }

    #[test]
    fn checkbox_and_radio_click_route_clicked() {
        for kind in [WindowsHandleKind::CheckBox, WindowsHandleKind::RadioButton] {
            assert_eq!(
                control_notify_kind_for_widget(kind, BN_CLICKED),
                Some(WidgetTriggerKind::Clicked),
                "{kind:?} should route BN_CLICKED to Clicked"
            );
        }
    }

    #[test]
    fn button_ignores_non_click_code() {
        assert_eq!(control_notify_kind_for_widget(WindowsHandleKind::Button, 0x0200), None);
    }

    #[test]
    fn line_edit_change_routes_value_changed() {
        assert_eq!(
            control_notify_kind_for_widget(WindowsHandleKind::LineEdit, EN_CHANGE),
            Some(WidgetTriggerKind::ValueChanged)
        );
    }

    #[test]
    fn combo_selchange_routes_selection_changed() {
        assert_eq!(
            control_notify_kind_for_widget(WindowsHandleKind::ComboBox, CBN_SELCHANGE),
            Some(WidgetTriggerKind::SelectionChanged)
        );
    }

    #[test]
    fn combo_editchange_routes_value_changed() {
        assert_eq!(
            control_notify_kind_for_widget(WindowsHandleKind::ComboBox, CBN_EDITCHANGE),
            Some(WidgetTriggerKind::ValueChanged)
        );
    }

    #[test]
    fn listbox_selchange_routes_selection_changed() {
        assert_eq!(
            control_notify_kind_for_widget(WindowsHandleKind::ListBox, LBN_SELCHANGE),
            Some(WidgetTriggerKind::SelectionChanged)
        );
    }

    #[test]
    fn unmapped_kind_yields_none() {
        assert_eq!(control_notify_kind_for_widget(WindowsHandleKind::Panel, BN_CLICKED), None);
        assert_eq!(control_notify_kind_for_widget(WindowsHandleKind::Label, EN_CHANGE), None);
    }

    #[test]
    fn combo_selchange_is_dual_emitted_but_editchange_is_not() {
        assert!(combo_selchange_is_dual_emitted(
            WindowsHandleKind::ComboBox,
            WidgetTriggerKind::SelectionChanged
        ));
        assert!(!combo_selchange_is_dual_emitted(
            WindowsHandleKind::ComboBox,
            WidgetTriggerKind::ValueChanged
        ));
        assert!(!combo_selchange_is_dual_emitted(
            WindowsHandleKind::ListBox,
            WidgetTriggerKind::SelectionChanged
        ));
    }

    #[test]
    fn slider_always_routes_value_changed() {
        assert_eq!(
            notify_kind_for_widget(WindowsHandleKind::Slider, 0),
            Some(WidgetTriggerKind::ValueChanged)
        );
        assert_eq!(
            notify_kind_for_widget(WindowsHandleKind::Slider, 1234),
            Some(WidgetTriggerKind::ValueChanged)
        );
    }

    #[test]
    fn zero_notify_code_without_slider_yields_none() {
        assert_eq!(notify_kind_for_widget(WindowsHandleKind::Button, 0), None);
    }
}
