//! Type-safe widget handles backed by `ObjectId`.
//!
//! Each handle type wraps a raw `ObjectId` and exposes only the operations
//! that are valid for that widget kind.  Handles also support event callbacks
//! via the `WidgetHandle` extension trait.

use alloc::rc::Rc;
use core::cell::RefCell;

use crate::core::{ObjectId, Orientation};
use crate::platform::WidgetTriggerKind;

// ═══════════════════════════════════════════════════════════════
// Supporting types used by widget handles
// ═══════════════════════════════════════════════════════════════

/// The visual state of a tri-state check-box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckState {
    /// Box is not checked.
    Unchecked,
    /// Box is checked.
    Checked,
    /// Box is in an indeterminate / partially-checked state.
    PartiallyChecked,
}

/// Controls how text is displayed in a line-edit widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EchoMode {
    /// Display characters as-is.
    Normal,
    /// Mask every character (e.g. for passwords).
    Password,
    /// Do not echo characters at all.
    NoEcho,
}

/// Determines how many rows can be selected in a list / table view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionMode {
    /// At most one row can be selected.
    Single,
    /// Multiple rows can be selected (toggle behaviour).
    Multi,
    /// Multiple rows can be selected with modifier keys (Ctrl/Shift).
    Extended,
    /// No row can be selected.
    None,
}

/// Data model interface for list / table views.
///
/// Widgets that display tabular data (such as [`ListViewHandle`]) use
/// a `Box<dyn ListModel>` to query the number of rows and the text for
/// each cell.
pub trait ListModel {
    /// Return the number of rows in the model.
    fn row_count(&self) -> usize;
    /// Return the text for the cell at `(row, col)`.
    fn text(&self, row: usize, col: usize) -> String;
    /// Update the text for the cell at `(row, col)`.
    fn set_text(&mut self, row: usize, col: usize, text: &str);
}

// ═══════════════════════════════════════════════════════════════
// Shared callback storage
// ═══════════════════════════════════════════════════════════════

/// Boxed callback invoked when a widget is triggered.
pub type ClickCallback = Rc<RefCell<dyn FnMut()>>;

/// Boxed callback invoked when a widget value changes.
pub type ValueChangedCallback = Rc<RefCell<dyn FnMut(String)>>;

// ═══════════════════════════════════════════════════════════════
// WidgetHandle trait – shared behaviour for all handles
// ═══════════════════════════════════════════════════════════════

/// Common operations available on every widget handle.
///
/// Implemented automatically by the `impl_handle!` macro and by
/// [`WindowHandle`].
pub trait WidgetHandle: Sized {
    /// Return the raw [`ObjectId`] backing this handle.
    fn raw_id(&self) -> ObjectId;

    /// Construct a handle from a raw [`ObjectId`].
    fn from_raw(id: ObjectId) -> Self;

    /// Show the widget.
    fn show(&self) {
        crate::show_widget(self.raw_id());
    }

    /// Hide the widget.
    fn hide(&self) {
        crate::hide_widget(self.raw_id());
    }

    /// Set widget geometry (position + size).
    fn set_geometry(&self, x: i32, y: i32, w: u32, h: u32) {
        crate::set_widget_geometry(self.raw_id(), x, y, w, h);
    }

    /// Update the widget's text / label.
    fn set_text(&self, text: &str) {
        crate::set_widget_text(self.raw_id(), text);
    }

    /// Read the widget's current text.
    fn text(&self) -> String {
        crate::get_widget_text(self.raw_id())
    }

    /// Enable the widget (accept user input).
    fn enable(&self) {
        crate::set_widget_enabled(self.raw_id(), true);
    }

    /// Disable the widget (ignore user input).
    fn disable(&self) {
        crate::set_widget_enabled(self.raw_id(), false);
    }

    /// Check whether the widget is currently enabled.
    fn is_enabled(&self) -> bool {
        crate::is_widget_enabled(self.raw_id())
    }

    /// Show or hide the widget.
    fn set_visible(&self, visible: bool) {
        crate::set_widget_visible(self.raw_id(), visible);
    }

    /// Check whether the widget is currently visible.
    fn is_visible(&self) -> bool {
        crate::is_widget_visible(self.raw_id())
    }

    /// Register a callback for the "clicked" trigger.
    ///
    /// The closure is invoked whenever the widget receives a
    /// [`WidgetTriggerKind::Clicked`] event.
    fn on_click<F: FnMut() + 'static>(&self, f: F);

    /// Register a callback for the "value changed" trigger.
    ///
    /// The closure receives the widget's current text at the time of
    /// the [`WidgetTriggerKind::ValueChanged`] event.
    fn on_value_changed<F: FnMut(String) + 'static>(&self, f: F);
}

// ── Global callback registry ──────────────────────────────────

use std::collections::HashMap;

thread_local! {
    static CLICK_CALLBACKS: RefCell<HashMap<ObjectId, ClickCallback>> = RefCell::new(HashMap::new());
    static VALUE_CALLBACKS: RefCell<HashMap<ObjectId, ValueChangedCallback>> = RefCell::new(HashMap::new());
}

/// Remove all registered callbacks for the given widget id.
///
/// Call this when a widget is destroyed to prevent callback leaks
/// from thread-local storage.
///
/// Uses `try_borrow_mut` to avoid panicking when called re-entrantly
/// (e.g. during callback dispatch when a handle is dropped).
pub fn remove_callbacks(id: ObjectId) {
    CLICK_CALLBACKS.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    VALUE_CALLBACKS.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
}

/// Dispatch a trigger event to the registered callback for `widget_id`.
/// Returns `true` if a callback was found and invoked.
///
/// The callback is **removed** from the map before invocation and then
/// **re-inserted** afterwards, so that re-entrant calls to
/// `remove_callbacks` (from a handle Drop inside the callback) do not
/// panic on a double borrow.
pub fn dispatch_trigger(widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
    match kind {
        WidgetTriggerKind::Clicked | WidgetTriggerKind::Unknown => {
            let cb = CLICK_CALLBACKS.with(|map| map.borrow_mut().remove(&widget_id));
            if let Some(cb) = cb {
                (cb.borrow_mut())();
                CLICK_CALLBACKS.with(|map| {
                    map.borrow_mut().insert(widget_id, cb);
                });
                true
            } else {
                false
            }
        }
        WidgetTriggerKind::ValueChanged | WidgetTriggerKind::SelectionChanged => {
            let text = crate::get_widget_text(widget_id);
            let cb = VALUE_CALLBACKS.with(|map| map.borrow_mut().remove(&widget_id));
            if let Some(cb) = cb {
                (cb.borrow_mut())(text);
                VALUE_CALLBACKS.with(|map| {
                    map.borrow_mut().insert(widget_id, cb);
                });
                true
            } else {
                false
            }
        }
        WidgetTriggerKind::Closed => {
            // Clean up callbacks when a widget is closed/destroyed.
            remove_callbacks(widget_id);
            false
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// WindowHandle
// ═══════════════════════════════════════════════════════════════

/// Type-safe handle for a top-level window.
///
/// In addition to the common widget operations, `WindowHandle` provides
/// factory methods for creating child widgets inside the window.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
}

impl WidgetHandle for WindowHandle {
    fn raw_id(&self) -> ObjectId {
        self.id
    }

    fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    /// Override `set_geometry` to store window size in `WindowState` for
    /// later use by `center_on_screen()` and other geometry-aware methods.
    fn set_geometry(&self, x: i32, y: i32, w: u32, h: u32) {
        WINDOW_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.id).or_insert_with(Default::default);
            state.x = x;
            state.y = y;
            state.w = w;
            state.h = h;
        });
        crate::set_widget_geometry(self.raw_id(), x, y, w, h);
    }

    fn on_click<F: FnMut() + 'static>(&self, f: F) {
        CLICK_CALLBACKS.with(|map| {
            map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
        });
    }

    fn on_value_changed<F: FnMut(String) + 'static>(&self, f: F) {
        VALUE_CALLBACKS.with(|map| {
            map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
        });
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        remove_callbacks(self.id);
    }
}

impl WindowHandle {
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

    pub fn new_radio_button(
        &self,
        text: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> RadioButtonHandle {
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

    pub fn new_frame(&self, x: i32, y: i32, w: u32, h: u32) -> FrameHandle {
        FrameHandle::from_raw(crate::create_panel(self.id, x, y, w, h))
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

    pub fn new_message_box(
        &self,
        title: &str,
        text: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> MessageBoxHandle {
        MessageBoxHandle::from_raw(crate::create_message_box(self.id, title, text, x, y, w, h))
    }

    /// Apply a layout manager to this window.
    ///
    /// The layout is stored internally and used to reposition children.
    /// Only one layout can be active at a time; calling this again replaces it.
    ///
    /// ```rust,no_run
    /// use rust_widgets::app::{App, WidgetHandle};
    /// use rust_widgets::layout::{BoxLayout, Layout, Orientation};
    ///
    /// let mut app = App::new();
    /// app.init();
    /// let win = app.new_window("Layout Demo", 0, 0, 400, 300);
    /// let btn1 = win.new_button("Left", 0, 0, 0, 0);
    /// let btn2 = win.new_button("Right", 0, 0, 0, 0);
    ///
    /// let mut layout = BoxLayout::new(Orientation::Horizontal, 8, 4);
    /// layout.add_widget(btn1.raw_id(), 1);
    /// layout.add_widget(btn2.raw_id(), 1);
    /// win.set_layout(layout);
    /// ```
    pub fn set_layout(&self, layout: impl crate::layout::Layout + 'static) {
        LAYOUTS.with(|map| {
            map.borrow_mut().insert(self.id, Box::new(layout));
        });
    }
}

thread_local! {
    static LAYOUTS: RefCell<HashMap<ObjectId, Box<dyn crate::layout::Layout>>> = RefCell::new(HashMap::new());
}

// ═══════════════════════════════════════════════════════════════
// Macro for standard widget handles
// ═══════════════════════════════════════════════════════════════

macro_rules! impl_handle {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            id: ObjectId,
        }

        impl $name {
            pub fn from_raw(id: ObjectId) -> Self {
                Self { id }
            }
        }

        impl WidgetHandle for $name {
            fn raw_id(&self) -> ObjectId {
                self.id
            }

            fn from_raw(id: ObjectId) -> Self {
                Self { id }
            }

            fn on_click<F: FnMut() + 'static>(&self, f: F) {
                CLICK_CALLBACKS.with(|map| {
                    map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
                });
            }

            fn on_value_changed<F: FnMut(String) + 'static>(&self, f: F) {
                VALUE_CALLBACKS.with(|map| {
                    map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
                });
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                remove_callbacks(self.id);
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
impl_handle!(TextEditHandle, "Type-safe handle for a TextEdit (multi-line text) widget.");
impl_handle!(ScrollBarHandle, "Type-safe handle for a ScrollBar widget.");
impl_handle!(TabWidgetHandle, "Type-safe handle for a TabWidget (tab container) widget.");
impl_handle!(GridWidgetHandle, "Type-safe handle for a GridWidget (grid layout) widget.");
impl_handle!(FrameHandle, "Type-safe handle for a Frame widget.");
impl_handle!(DialogHandle, "Type-safe handle for a generic Dialog widget.");
impl_handle!(WebViewHandle, "Type-safe handle for a WebView (web content) widget.");

// ═══════════════════════════════════════════════════════════════
// MessageBoxHandle – custom, NOT from macro
// ═══════════════════════════════════════════════════════════════

/// Type-safe handle for a modal message-box dialog.
///
/// Unlike normal widgets, a message-box exposes only dialog-oriented
/// operations — it does **not** support `set_text`, `enable`, or
/// `set_geometry` because those semantics do not apply.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageBoxHandle {
    id: ObjectId,
}

impl MessageBoxHandle {
    pub fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    pub fn raw_id(&self) -> ObjectId {
        self.id
    }

    /// Show the message-box modally.
    pub fn show_modal(&self) {
        crate::show_widget(self.id);
    }

    /// Dismiss the message-box.
    pub fn close(&self) {
        crate::hide_widget(self.id);
    }

    /// Update the dialog title.
    pub fn set_title(&self, title: &str) {
        crate::set_widget_text(self.id, title);
    }
}

impl WidgetHandle for MessageBoxHandle {
    fn raw_id(&self) -> ObjectId {
        self.id
    }

    fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    fn on_click<F: FnMut() + 'static>(&self, f: F) {
        CLICK_CALLBACKS.with(|map| {
            map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
        });
    }

    fn on_value_changed<F: FnMut(String) + 'static>(&self, f: F) {
        VALUE_CALLBACKS.with(|map| {
            map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
        });
    }
}

impl Drop for MessageBoxHandle {
    fn drop(&mut self) {
        remove_callbacks(self.id);
    }
}

// ═══════════════════════════════════════════════════════════════
// ComboBoxHandle – extended with combo-specific operations
// ═══════════════════════════════════════════════════════════════

/// # Combo-box specific operations
impl ComboBoxHandle {
    pub fn add_item(&self, text: &str) -> bool {
        crate::combo_box_add_item(self.raw_id(), text)
    }

    pub fn clear_items(&self) -> bool {
        crate::combo_box_clear_items(self.raw_id())
    }

    pub fn set_current_index(&self, index: usize) -> bool {
        crate::combo_box_set_current_index(self.raw_id(), index)
    }

    pub fn current_index(&self) -> Option<usize> {
        crate::combo_box_current_index(self.raw_id())
    }

    pub fn item_count(&self) -> usize {
        crate::combo_box_item_count(self.raw_id())
    }

    pub fn item_text(&self, index: usize) -> Option<String> {
        crate::combo_box_item_text(self.raw_id(), index)
    }
}

// ═══════════════════════════════════════════════════════════════
// ListBoxHandle – extended with list-specific operations
// ═══════════════════════════════════════════════════════════════

/// # List-box specific operations
impl ListBoxHandle {
    pub fn add_item(&self, text: &str) -> bool {
        crate::list_box_add_item(self.raw_id(), text)
    }

    pub fn remove_item(&self, index: usize) -> bool {
        crate::list_box_remove_item(self.raw_id(), index)
    }

    pub fn clear_items(&self) -> bool {
        crate::list_box_clear_items(self.raw_id())
    }

    pub fn set_current_index(&self, index: usize) -> bool {
        crate::list_box_set_current_index(self.raw_id(), index)
    }

    pub fn current_index(&self) -> Option<usize> {
        crate::list_box_current_index(self.raw_id())
    }

    pub fn item_count(&self) -> usize {
        crate::list_box_item_count(self.raw_id())
    }

    pub fn item_text(&self, index: usize) -> Option<String> {
        crate::list_box_item_text(self.raw_id(), index)
    }
}

// ═══════════════════════════════════════════════════════════════
// SliderHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct SliderState {
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    orientation: Orientation,
}

impl Default for SliderState {
    fn default() -> Self {
        Self { value: 50, min: 0, max: 100, step: 1, orientation: Orientation::Horizontal }
    }
}

thread_local! {
    static SLIDER_STATES: RefCell<HashMap<ObjectId, SliderState>> = RefCell::new(HashMap::new());
}

/// # Slider-specific operations
impl SliderHandle {
    /// Set the current slider value (clamped to min/max range).
    pub fn set_value(&self, value: i32) {
        SLIDER_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.value = value.clamp(state.min, state.max);
        });
    }

    /// Return the current slider value.
    pub fn value(&self) -> i32 {
        SLIDER_STATES.with(|map| map.borrow().get(&self.raw_id()).map(|s| s.value).unwrap_or(50))
    }

    /// Set the slider range (min/max). The current value is clamped.
    pub fn set_range(&self, min: i32, max: i32) {
        SLIDER_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.min = min;
            state.max = max;
            state.value = state.value.clamp(state.min, state.max);
        });
    }

    /// Set the slider step increment.
    pub fn set_step(&self, step: i32) {
        SLIDER_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).step = step;
        });
    }

    /// Set the slider orientation.
    pub fn set_orientation(&self, orientation: Orientation) {
        SLIDER_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).orientation =
                orientation;
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// ProgressBarHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct ProgressBarState {
    value: u32,
    min: u32,
    max: u32,
    indeterminate: bool,
}

impl Default for ProgressBarState {
    fn default() -> Self {
        Self { value: 0, min: 0, max: 100, indeterminate: false }
    }
}

thread_local! {
    static PROGRESS_BAR_STATES: RefCell<HashMap<ObjectId, ProgressBarState>> = RefCell::new(HashMap::new());
}

/// # Progress-bar specific operations
impl ProgressBarHandle {
    /// Set the current progress value (clamped to min/max).
    pub fn set_value(&self, value: u32) {
        PROGRESS_BAR_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.value = value.clamp(state.min, state.max);
        });
    }

    /// Return the current progress value.
    pub fn value(&self) -> u32 {
        PROGRESS_BAR_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.value).unwrap_or(0))
    }

    /// Set the minimum value.
    pub fn set_min(&self, min: u32) {
        PROGRESS_BAR_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.min = min;
            state.value = state.value.clamp(state.min, state.max);
        });
    }

    /// Set the maximum value.
    pub fn set_max(&self, max: u32) {
        PROGRESS_BAR_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.max = max;
            state.value = state.value.clamp(state.min, state.max);
        });
    }

    /// Set whether the progress bar is in indeterminate mode.
    pub fn set_indeterminate(&self, indeterminate: bool) {
        PROGRESS_BAR_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).indeterminate =
                indeterminate;
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// CheckBoxHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct CheckBoxState {
    checked: bool,
    tristate: bool,
    check_state: CheckState,
}

impl Default for CheckBoxState {
    fn default() -> Self {
        Self { checked: false, tristate: false, check_state: CheckState::Unchecked }
    }
}

thread_local! {
    static CHECKBOX_STATES: RefCell<HashMap<ObjectId, CheckBoxState>> = RefCell::new(HashMap::new());
}

/// # Check-box specific operations
impl CheckBoxHandle {
    /// Return whether the check-box is checked (non-tristate mode).
    pub fn is_checked(&self) -> bool {
        CHECKBOX_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.checked).unwrap_or(false))
    }

    /// Set the check-box to checked or unchecked.
    pub fn set_checked(&self, checked: bool) {
        CHECKBOX_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.checked = checked;
            if !state.tristate {
                state.check_state =
                    if checked { CheckState::Checked } else { CheckState::Unchecked };
            }
        });
    }

    /// Enable/disable tri-state mode.
    pub fn set_tristate(&self, tristate: bool) {
        CHECKBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).tristate =
                tristate;
        });
    }

    /// Return the current check state of a tri-state check-box.
    pub fn check_state(&self) -> CheckState {
        CHECKBOX_STATES.with(|map| {
            map.borrow().get(&self.raw_id()).map(|s| s.check_state).unwrap_or(CheckState::Unchecked)
        })
    }
}

// ═══════════════════════════════════════════════════════════════
// RadioButtonHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
struct RadioButtonState {
    selected: bool,
    group: String,
}

thread_local! {
    static RADIO_BUTTON_STATES: RefCell<HashMap<ObjectId, RadioButtonState>> = RefCell::new(HashMap::new());
}

/// # Radio-button specific operations
impl RadioButtonHandle {
    /// Return whether this radio button is currently selected.
    pub fn is_selected(&self) -> bool {
        RADIO_BUTTON_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.selected).unwrap_or(false))
    }

    /// Select this radio button and deselect all others in the same group.
    pub fn select(&self) {
        let group = RADIO_BUTTON_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.selected = true;
            state.group.clone()
        });

        // Deselect all other radio buttons in the same group.
        if !group.is_empty() {
            RADIO_BUTTON_STATES.with(|map| {
                let mut map = map.borrow_mut();
                for (id, state) in map.iter_mut() {
                    if *id != self.raw_id() && state.group == group {
                        state.selected = false;
                    }
                }
            });
        }
    }

    /// Set the group name for this radio button.
    /// Radio buttons in the same group are mutually exclusive.
    pub fn set_group(&self, group: &str) {
        RADIO_BUTTON_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).group =
                group.to_owned();
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// LineEditHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct LineEditState {
    placeholder: String,
    read_only: bool,
    max_length: u32,
    echo_mode: EchoMode,
    selection_start: u32,
    selection_end: u32,
    select_all: bool,
}

impl Default for LineEditState {
    fn default() -> Self {
        Self {
            placeholder: String::new(),
            read_only: false,
            max_length: 32767,
            echo_mode: EchoMode::Normal,
            selection_start: 0,
            selection_end: 0,
            select_all: false,
        }
    }
}

thread_local! {
    static LINE_EDIT_STATES: RefCell<HashMap<ObjectId, LineEditState>> = RefCell::new(HashMap::new());
}

/// # Line-edit specific operations
impl LineEditHandle {
    /// Set the placeholder text shown when the field is empty.
    pub fn set_placeholder(&self, text: &str) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).placeholder =
                text.to_owned();
        });
    }

    /// Set whether the line-edit is read-only.
    pub fn set_read_only(&self, read_only: bool) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).read_only =
                read_only;
        });
    }

    /// Set the maximum number of characters allowed.
    pub fn set_max_length(&self, len: u32) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).max_length = len;
        });
    }

    /// Clear the line-edit text.
    pub fn clear(&self) {
        crate::set_widget_text(self.raw_id(), "");
    }

    /// Set the echo mode (Normal / Password / NoEcho).
    pub fn set_echo_mode(&self, mode: EchoMode) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).echo_mode = mode;
        });
    }

    /// Select all text in the line-edit.
    pub fn select_all(&self) {
        LINE_EDIT_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.select_all = true;
            state.selection_start = 0;
            state.selection_end = u32::MAX;
        });
    }

    /// Set the selection range (start..end).
    pub fn set_selection(&self, start: u32, end: u32) {
        LINE_EDIT_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.select_all = false;
            state.selection_start = start;
            state.selection_end = end;
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// ScrollAreaHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
struct ScrollAreaState {
    scroll_x: i32,
    scroll_y: i32,
    content_w: u32,
    content_h: u32,
}

thread_local! {
    static SCROLL_AREA_STATES: RefCell<HashMap<ObjectId, ScrollAreaState>> = RefCell::new(HashMap::new());
}

/// # Scroll-area specific operations
impl ScrollAreaHandle {
    /// Set the scroll offset.
    pub fn set_scroll_position(&self, x: i32, y: i32) {
        SCROLL_AREA_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.scroll_x = x;
            state.scroll_y = y;
        });
    }

    /// Return the current scroll offset.
    pub fn scroll_position(&self) -> (i32, i32) {
        SCROLL_AREA_STATES.with(|map| {
            let map = map.borrow();
            map.get(&self.raw_id()).map(|s| (s.scroll_x, s.scroll_y)).unwrap_or((0, 0))
        })
    }

    /// Set the content size (in virtual pixels).
    pub fn set_content_size(&self, w: u32, h: u32) {
        SCROLL_AREA_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.content_w = w;
            state.content_h = h;
        });
    }

    /// Scroll to the bottom of the content.
    pub fn scroll_to_bottom(&self) {
        SCROLL_AREA_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.get(&self.raw_id()).cloned().unwrap_or_default();
            let state_mut = map.entry(self.raw_id()).or_default();
            state_mut.scroll_y = state.content_h as i32;
        });
    }

    /// Scroll to the top of the content.
    pub fn scroll_to_top(&self) {
        SCROLL_AREA_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).scroll_y = 0;
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// ListViewHandle – extended state
// ═══════════════════════════════════════════════════════════════

struct ListViewState {
    columns: Vec<(String, u32)>,
    model: Option<Rc<dyn ListModel>>,
    selected_row: Option<usize>,
    selection_mode: SelectionMode,
}

impl Default for ListViewState {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            model: None,
            selected_row: None,
            selection_mode: SelectionMode::Single,
        }
    }
}

thread_local! {
    static LIST_VIEW_STATES: RefCell<HashMap<ObjectId, ListViewState>> = RefCell::new(HashMap::new());
}

/// # List-view specific operations
impl ListViewHandle {
    /// Add a column with the given title and width.
    pub fn add_column(&self, title: &str, width: u32) {
        LIST_VIEW_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.columns.push((title.to_owned(), width));
        });
    }

    /// Set the data model for this list view.
    pub fn set_model(&self, model: Box<dyn ListModel>) {
        let model: Rc<dyn ListModel> = Rc::from(model);
        LIST_VIEW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).model =
                Some(model);
        });
    }

    /// Return the currently selected row, if any.
    pub fn selected_row(&self) -> Option<usize> {
        LIST_VIEW_STATES.with(|map| map.borrow().get(&self.raw_id()).and_then(|s| s.selected_row))
    }

    /// Set the selection mode.
    pub fn set_selection_mode(&self, mode: SelectionMode) {
        LIST_VIEW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).selection_mode =
                mode;
        });
    }

    /// Return the current model, if one is set.
    /// Returns `None` if no model has been assigned.
    pub fn model(&self) -> Option<Rc<dyn ListModel>> {
        LIST_VIEW_STATES.with(|map| map.borrow().get(&self.raw_id()).and_then(|s| s.model.clone()))
    }
}

// ═══════════════════════════════════════════════════════════════
// SpinBoxHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct SpinBoxState {
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    prefix: String,
    suffix: String,
}

impl Default for SpinBoxState {
    fn default() -> Self {
        Self { value: 0, min: 0, max: 100, step: 1, prefix: String::new(), suffix: String::new() }
    }
}

thread_local! {
    static SPINBOX_STATES: RefCell<HashMap<ObjectId, SpinBoxState>> = RefCell::new(HashMap::new());
}

/// # Spin-box specific operations
impl SpinBoxHandle {
    /// Set the current spin-box value (clamped to range).
    pub fn set_value(&self, value: i32) {
        SPINBOX_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.value = value.clamp(state.min, state.max);
        });
    }

    /// Return the current spin-box value.
    pub fn value(&self) -> i32 {
        SPINBOX_STATES.with(|map| map.borrow().get(&self.raw_id()).map(|s| s.value).unwrap_or(0))
    }

    /// Set the spin-box range. The current value is clamped.
    pub fn set_range(&self, min: i32, max: i32) {
        SPINBOX_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.min = min;
            state.max = max;
            state.value = state.value.clamp(state.min, state.max);
        });
    }

    /// Set the prefix text displayed before the value.
    pub fn set_prefix(&self, prefix: &str) {
        SPINBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).prefix =
                prefix.to_owned();
        });
    }

    /// Set the suffix text displayed after the value.
    pub fn set_suffix(&self, suffix: &str) {
        SPINBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).suffix =
                suffix.to_owned();
        });
    }

    /// Set the spin-box step increment.
    pub fn set_step(&self, step: i32) {
        SPINBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).step = step;
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// PanelHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
struct PanelState {
    title: String,
}

thread_local! {
    static PANEL_LAYOUTS: RefCell<HashMap<ObjectId, Box<dyn crate::layout::Layout>>> = RefCell::new(HashMap::new());
    static PANEL_STATES: RefCell<HashMap<ObjectId, PanelState>> = RefCell::new(HashMap::new());
}

/// # Panel-specific operations
impl PanelHandle {
    /// Set the layout manager for this panel.
    ///
    /// The layout is stored internally and used to reposition children.
    /// Only one layout can be active at a time.
    pub fn set_layout(&self, layout: Box<dyn crate::layout::Layout>) {
        PANEL_LAYOUTS.with(|map| {
            map.borrow_mut().insert(self.raw_id(), layout);
        });
    }

    /// Set the panel title text.
    pub fn set_title(&self, title: &str) {
        PANEL_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).title =
                title.to_owned();
        });
        crate::set_widget_text(self.raw_id(), title);
    }
}

// ═══════════════════════════════════════════════════════════════
// WindowHandle – extended state (new methods beyond factories)
// ═══════════════════════════════════════════════════════════════

#[derive(Clone)]
struct WindowState {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    icon: String,
    min_w: u32,
    min_h: u32,
    maximized: bool,
    minimized: bool,
    fullscreen: bool,
    resizable: bool,
    decorated: bool,
    close_callback: Option<ClickCallback>,
}

impl std::fmt::Debug for WindowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowState")
            .field("x", &self.x)
            .field("y", &self.y)
            .field("w", &self.w)
            .field("h", &self.h)
            .field("icon", &self.icon)
            .field("min_w", &self.min_w)
            .field("min_h", &self.min_h)
            .field("maximized", &self.maximized)
            .field("minimized", &self.minimized)
            .field("fullscreen", &self.fullscreen)
            .field("resizable", &self.resizable)
            .field("decorated", &self.decorated)
            .field("close_callback", &self.close_callback.as_ref().map(|_| "<fn>"))
            .finish()
    }
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            w: 800,
            h: 600,
            icon: String::new(),
            min_w: 0,
            min_h: 0,
            maximized: false,
            minimized: false,
            fullscreen: false,
            resizable: true,
            decorated: true,
            close_callback: None,
        }
    }
}

thread_local! {
    static WINDOW_STATES: RefCell<HashMap<ObjectId, WindowState>> = RefCell::new(HashMap::new());
}

/// # Window-specific state operations
impl WindowHandle {
    /// Return the window title.
    pub fn title(&self) -> String {
        crate::get_widget_text(self.raw_id())
    }

    /// Set the window icon from a file path.
    pub fn set_icon(&self, path: &str) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).icon =
                path.to_owned();
        });
    }

    /// Set the minimum window size.
    pub fn set_min_size(&self, w: u32, h: u32) {
        WINDOW_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.min_w = w;
            state.min_h = h;
        });
    }

    /// Maximize or restore the window.
    pub fn set_maximized(&self, maximized: bool) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).maximized =
                maximized;
        });
    }

    /// Return whether the window is maximized.
    pub fn is_maximized(&self) -> bool {
        WINDOW_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.maximized).unwrap_or(false))
    }

    /// Minimize or restore the window.
    pub fn set_minimized(&self, minimized: bool) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).minimized =
                minimized;
        });
    }

    /// Return whether the window is minimized.
    pub fn is_minimized(&self) -> bool {
        WINDOW_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.minimized).unwrap_or(false))
    }

    /// Set fullscreen mode.
    pub fn set_fullscreen(&self, fullscreen: bool) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).fullscreen =
                fullscreen;
        });
    }

    /// Return whether the window is fullscreen.
    pub fn is_fullscreen(&self) -> bool {
        WINDOW_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.fullscreen).unwrap_or(false))
    }

    /// Set whether the window is resizable.
    pub fn set_resizable(&self, resizable: bool) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).resizable =
                resizable;
        });
    }

    /// Return whether the window is resizable.
    pub fn is_resizable(&self) -> bool {
        WINDOW_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.resizable).unwrap_or(true))
    }

    /// Set whether the window has window decorations (title bar, borders).
    pub fn set_decorated(&self, decorated: bool) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).decorated =
                decorated;
        });
    }

    /// Is the window decorated?
    pub fn is_decorated(&self) -> bool {
        WINDOW_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.decorated).unwrap_or(true))
    }

    /// Register a callback invoked when the window is about to close.
    pub fn on_close(&self, callback: ClickCallback) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).close_callback =
                Some(callback);
        });
    }

    /// Programmatically close the window.
    pub fn close(&self) {
        // Invoke the close callback if one is registered.
        WINDOW_STATES.with(|map| {
            let mut map = map.borrow_mut();
            if let Some(state) = map.get_mut(&self.raw_id()) {
                if let Some(cb) = &state.close_callback {
                    (cb.borrow_mut())();
                }
            }
        });
        crate::hide_widget(self.raw_id());
    }

    /// Center the window on the screen.
    ///
    /// Uses a default virtual screen size of 1920×1080 as a fallback.
    /// Real platforms should query the actual screen geometry.
    pub fn center_on_screen(&self) {
        // Query the stored window geometry to preserve current size.
        let (win_w, win_h) = WINDOW_STATES.with(|map| {
            let state = map.borrow().get(&self.raw_id()).cloned().unwrap_or_default();
            (state.w, state.h)
        });
        let screen_w = 1920i32;
        let screen_h = 1080i32;
        crate::set_widget_geometry(
            self.raw_id(),
            (screen_w - win_w as i32) / 2,
            (screen_h - win_h as i32) / 2,
            win_w,
            win_h,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ObjectId;
    use alloc::rc::Rc;
    use core::cell::RefCell;

    #[test]
    fn remove_callbacks_cleans_up() {
        let id: ObjectId = 42;
        CLICK_CALLBACKS.with(|map| {
            map.borrow_mut().insert(id, Rc::new(RefCell::new(|| {})));
            assert!(map.borrow().contains_key(&id));
        });
        remove_callbacks(id);
        CLICK_CALLBACKS.with(|map| {
            assert!(!map.borrow().contains_key(&id));
        });
    }
}
