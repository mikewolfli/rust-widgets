//! Type-safe widget handles backed by `ObjectId`.
//!
//! Each handle type wraps a raw `ObjectId` and exposes only the operations
//! that are valid for that widget kind.

use crate::core::ObjectId;

// ═══════════════════════════════════════════════════════════════
// WindowHandle
// ═══════════════════════════════════════════════════════════════

/// Type-safe handle for a top-level window.
///
/// In addition to the common widget operations, `WindowHandle` provides
/// factory methods for creating child widgets inside the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowHandle {
    id: ObjectId,
}

impl WindowHandle {
    pub fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    pub fn raw_id(&self) -> ObjectId {
        self.id
    }

    pub fn show(&self) {
        crate::show_widget(self.id);
    }

    pub fn hide(&self) {
        crate::hide_widget(self.id);
    }

    pub fn set_geometry(&self, x: i32, y: i32, w: u32, h: u32) {
        crate::set_widget_geometry(self.id, x, y, w, h);
    }

    pub fn set_title(&self, title: &str) {
        crate::set_widget_text(self.id, title);
    }

    // ── Child-widget factory methods ──────────────────────

    pub fn new_button(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> ButtonHandle {
        ButtonHandle::from_raw(crate::create_button(self.id, text, x, y, w, h))
    }

    pub fn new_label(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> LabelHandle {
        LabelHandle::from_raw(crate::create_label(self.id, text, x, y, w, h))
    }

    pub fn new_checkbox(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> CheckBoxHandle {
        CheckBoxHandle::from_raw(crate::create_checkbox(self.id, text, x, y, w, h))
    }

    pub fn new_radio_button(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> RadioButtonHandle {
        RadioButtonHandle::from_raw(crate::create_radio_button(self.id, text, x, y, w, h))
    }

    pub fn new_line_edit(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> LineEditHandle {
        LineEditHandle::from_raw(crate::create_line_edit(self.id, text, x, y, w, h))
    }

    pub fn new_combo_box(&self, x: i32, y: i32, w: u32, h: u32) -> ComboBoxHandle {
        ComboBoxHandle::from_raw(crate::create_combo_box(self.id, x, y, w, h))
    }

    pub fn new_list_box(&self, x: i32, y: i32, w: u32, h: u32) -> ListBoxHandle {
        ListBoxHandle::from_raw(crate::create_list_box(self.id, x, y, w, h))
    }

    pub fn new_slider(&self, x: i32, y: i32, w: u32, h: u32) -> SliderHandle {
        SliderHandle::from_raw(crate::create_slider(self.id, x, y, w, h))
    }

    pub fn new_progress_bar(&self, x: i32, y: i32, w: u32, h: u32) -> ProgressBarHandle {
        ProgressBarHandle::from_raw(crate::create_progress_bar(self.id, x, y, w, h))
    }

    pub fn new_panel(&self, x: i32, y: i32, w: u32, h: u32) -> PanelHandle {
        PanelHandle::from_raw(crate::create_panel(self.id, x, y, w, h))
    }

    pub fn new_spin_box(&self, x: i32, y: i32, w: u32, h: u32) -> SpinBoxHandle {
        SpinBoxHandle::from_raw(crate::create_spin_box(self.id, x, y, w, h))
    }

    pub fn new_list_view(&self, x: i32, y: i32, w: u32, h: u32) -> ListViewHandle {
        ListViewHandle::from_raw(crate::create_list_view(self.id, x, y, w, h))
    }

    pub fn new_scroll_area(&self, x: i32, y: i32, w: u32, h: u32) -> ScrollAreaHandle {
        ScrollAreaHandle::from_raw(crate::create_scroll_area(self.id, x, y, w, h))
    }
}

// ═══════════════════════════════════════════════════════════════
// Macro-generated widget handles
// ═══════════════════════════════════════════════════════════════

macro_rules! impl_handle {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name {
            id: ObjectId,
        }

        impl $name {
            pub fn from_raw(id: ObjectId) -> Self {
                Self { id }
            }

            pub fn raw_id(&self) -> ObjectId {
                self.id
            }

            pub fn show(&self) {
                crate::show_widget(self.id);
            }

            pub fn hide(&self) {
                crate::hide_widget(self.id);
            }

            pub fn set_geometry(&self, x: i32, y: i32, w: u32, h: u32) {
                crate::set_widget_geometry(self.id, x, y, w, h);
            }

            pub fn set_text(&self, text: &str) {
                crate::set_widget_text(self.id, text);
            }

            pub fn text(&self) -> String {
                crate::get_widget_text(self.id)
            }

            pub fn enable(&self) {
                crate::set_widget_enabled(self.id, true);
            }

            pub fn disable(&self) {
                crate::set_widget_enabled(self.id, false);
            }

            pub fn is_enabled(&self) -> bool {
                crate::is_widget_enabled(self.id)
            }

            pub fn set_visible(&self, visible: bool) {
                crate::set_widget_visible(self.id, visible);
            }

            pub fn is_visible(&self) -> bool {
                crate::is_widget_visible(self.id)
            }
        }
    };
}

impl_handle!(ButtonHandle, "Type-safe handle for a Button widget.");
impl_handle!(LabelHandle, "Type-safe handle for a Label widget.");
impl_handle!(CheckBoxHandle, "Type-safe handle for a CheckBox widget.");
impl_handle!(RadioButtonHandle, "Type-safe handle for a RadioButton widget.");
impl_handle!(LineEditHandle, "Type-safe handle for a LineEdit widget.");
impl_handle!(ComboBoxHandle, "Type-safe handle for a ComboBox widget.");
impl_handle!(ListBoxHandle, "Type-safe handle for a ListBox widget.");
impl_handle!(SliderHandle, "Type-safe handle for a Slider widget.");
impl_handle!(ProgressBarHandle, "Type-safe handle for a ProgressBar widget.");
impl_handle!(PanelHandle, "Type-safe handle for a Panel widget.");
impl_handle!(SpinBoxHandle, "Type-safe handle for a SpinBox widget.");
impl_handle!(ListViewHandle, "Type-safe handle for a ListView widget.");
impl_handle!(ScrollAreaHandle, "Type-safe handle for a ScrollArea widget.");
impl_handle!(MessageBoxHandle, "Type-safe handle for a MessageBox dialog.");
