//! Widget models and controls.

use crate::core::Rect;
use crate::object::Object;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;

// Base widget types
pub mod base;

// Widget subfolders
pub mod advanced_widgets;
pub mod base_widgets;
pub mod container_widgets;
pub mod dialog;
pub mod display_widgets;
pub mod input_widgets;
pub mod menu_toolbar;
pub mod special_widgets;
pub mod view_widgets;
pub mod web_widgets;

// Individual widget files (not in subfolders)
pub mod window;

// Re-export base types
pub use base::{BaseWidget, Draw, Image, Widget, WidgetKind};

// Re-export widget types from subfolders
pub use base_widgets::{
    button::{Button, ButtonState},
    checkbox::{CheckBox, CheckState},
    label::Label,
    radiobutton::RadioButton,
    toggle_button::{ToggleButton, ToggleButtonState},
};

pub use input_widgets::{
    combobox::ComboBox,
    lineedit::{EchoMode, LineEdit},
    listbox::{ListBox, SelectionMode},
    rich_edit::RichEdit,
    spinbox::SpinBox,
    textedit::TextEdit,
};

// Re-export container widgets
pub use container_widgets::{
    dockwidget::DockWidget,
    groupbox::GroupBox,
    mdiarea::MdiArea,
    scrollarea::ScrollArea,
    splitter::{Splitter, SplitterOrientation},
    stackedwidget::StackedWidget,
    tabwidget::TabWidget,
    toolbox::ToolBox,
};

// Re-export display widgets
pub use display_widgets::{
    lcd_number::LcdNumber, progressbar::ProgressBar, scrollbar::ScrollBar, slider::Slider,
};

// Re-export web widgets
pub use web_widgets::{web_engine::WebEngine, web_view::WebView};

// Re-export advanced widgets
pub use advanced_widgets::{
    calendar::Calendar, date_edit::DateEdit, date_time_edit::DateTimeEdit, dial::Dial,
    key_sequence_edit::KeySequenceEdit, time_edit::TimeEdit,
};

// Re-export dialog widgets
pub use dialog::{
    color_dialog::ColorDialog, file_dialog::FileDialog, font_dialog::FontDialog,
    input_dialog::InputDialog, message_box::MessageBox, popup_window::PopupWindow,
    progress_dialog::ProgressDialog,
};

// Re-export menu and toolbar widgets
pub use menu_toolbar::{
    action::Action, menu::Menu, menu_bar::MenuBar, status_bar::StatusBar, tool_bar::ToolBar,
    tool_button::ToolButton,
};

// Re-export view widgets
pub use view_widgets::{
    list_view::{ListModel, ListView, SelectionMode, VecListModel},
    table_view::TableView,
    table_widget::TableWidget,
    tree_view::TreeView,
};

// Re-export special widgets
pub use special_widgets::{Canvas, ChartWidget, GridWidget};

impl BaseWidget {
    /// Create base widget state and core signals.
    pub fn new(kind: WidgetKind, geometry: Rect, class_name: &'static str) -> Self {
        Self {
            object: Object::new(class_name),
            kind,
            geometry,
            min_size: None,
            max_size: None,
            parent: None,
            children: Vec::new(),
            visible: true,
            enabled: true,
            tooltip: String::new(),
            style: WidgetStyle::default(),
            connection_scope: ConnectionScope::new(),
            clicked: GenericSignal::new(),
            changed: GenericSignal::new(),
            hover: Signal1::new(),
            mouse_down: Signal1::new(),
            mouse_up: Signal1::new(),
            key_down: Signal1::new(),
            key_up: Signal1::new(),
            focus_gained: GenericSignal::new(),
            focus_lost: GenericSignal::new(),
            redraw_requested: GenericSignal::new(),
            layout_requested: GenericSignal::new(),
        }
    }
}

impl crate::widget::base::Widget for BaseWidget {
    fn id(&self) -> crate::object::ObjectId {
        self.object.id()
    }
    fn kind(&self) -> WidgetKind {
        self.kind
    }
    fn geometry(&self) -> Rect {
        self.geometry
    }
    fn set_geometry(&mut self, geometry: Rect) {
        let new_geometry =
            Rect::from_position_size(geometry.position(), self.constrained_size(geometry.size()));
        if self.geometry != new_geometry {
            self.geometry = new_geometry;
            self.redraw_requested.emit();
            self.layout_requested.emit();
        }
    }
    fn min_size(&self) -> Option<crate::core::Size> {
        self.min_size
    }
    fn set_min_size(&mut self, size: Option<crate::core::Size>) {
        self.min_size = size;
    }
    fn max_size(&self) -> Option<crate::core::Size> {
        self.max_size
    }
    fn set_max_size(&mut self, size: Option<crate::core::Size>) {
        self.max_size = size;
    }
    fn parent(&self) -> Option<crate::object::ObjectId> {
        self.parent
    }
    fn set_parent(&mut self, parent: Option<crate::object::ObjectId>) {
        self.parent = parent;
    }
    fn children(&self) -> &[crate::object::ObjectId] {
        &self.children
    }
    fn add_child(&mut self, child: crate::object::ObjectId) {
        self.children.push(child);
    }
    fn remove_child(&mut self, child: crate::object::ObjectId) -> bool {
        if let Some(index) = self.children.iter().position(|id| *id == child) {
            self.children.remove(index);
            true
        } else {
            false
        }
    }
    fn is_visible(&self) -> bool {
        self.visible
    }
    fn set_visible(&mut self, visible: bool) {
        if self.visible != visible {
            self.visible = visible;
            self.redraw_requested.emit();
        }
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.redraw_requested.emit();
        }
    }
    fn tooltip(&self) -> &str {
        &self.tooltip
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }
    fn style(&self) -> &WidgetStyle {
        &self.style
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.style = style;
        self.redraw_requested.emit();
    }
    fn connection_scope(&self) -> &ConnectionScope {
        &self.connection_scope
    }
    fn clicked_signal(&self) -> &GenericSignal {
        &self.clicked
    }
    fn changed_signal(&self) -> &GenericSignal {
        &self.changed
    }
    fn hover_signal(&self) -> &Signal1<crate::event::MouseEvent> {
        &self.hover
    }
    fn mouse_down_signal(&self) -> &Signal1<crate::event::MouseEvent> {
        &self.mouse_down
    }
    fn mouse_up_signal(&self) -> &Signal1<crate::event::MouseEvent> {
        &self.mouse_up
    }
    fn key_down_signal(&self) -> &Signal1<crate::event::KeyEvent> {
        &self.key_down
    }
    fn key_up_signal(&self) -> &Signal1<crate::event::KeyEvent> {
        &self.key_up
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        &self.focus_gained
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        &self.focus_lost
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        &self.redraw_requested
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        &self.layout_requested
    }
}

impl crate::event::EventHandler for BaseWidget {
    fn handle_event(&mut self, event: &crate::event::Event) -> bool {
        match event {
            crate::event::Event::MouseDown(mouse_event) => {
                self.mouse_down.emit(mouse_event.clone());
                true
            }
            crate::event::Event::MouseUp(mouse_event) => {
                self.mouse_up.emit(mouse_event.clone());
                true
            }
            crate::event::Event::MouseMove(mouse_event) => {
                self.hover.emit(mouse_event.clone());
                true
            }
            crate::event::Event::KeyDown(key_event) => {
                self.key_down.emit(key_event.clone());
                true
            }
            crate::event::Event::KeyUp(key_event) => {
                self.key_up.emit(key_event.clone());
                true
            }
            crate::event::Event::FocusGained => {
                self.focus_gained.emit();
                true
            }
            crate::event::Event::FocusLost => {
                self.focus_lost.emit();
                true
            }
            _ => false,
        }
    }
}

impl BaseWidget {
    /// Returns constrained size respecting min/max size limits.
    pub fn constrained_size(&self, size: crate::core::Size) -> crate::core::Size {
        let mut result = size;
        if let Some(min) = self.min_size {
            result.width = result.width.max(min.width);
            result.height = result.height.max(min.height);
        }
        if let Some(max) = self.max_size {
            result.width = result.width.min(max.width);
            result.height = result.height.min(max.height);
        }
        result
    }

    /// Requests a redraw of this widget.
    pub fn request_redraw(&self) {
        self.redraw_requested.emit();
    }

    /// Requests a layout update for this widget.
    pub fn request_layout(&self) {
        self.layout_requested.emit();
    }
}
