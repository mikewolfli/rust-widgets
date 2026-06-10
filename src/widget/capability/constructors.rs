use crate::core::{Point, Rect};
#[cfg(not(feature = "mini"))]
use crate::widget::special_widgets::freeform_shape::ShapePath;
use crate::widget::*;
use std::boxed::Box;

pub fn create_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Button::new(text.to_string(), geometry))
}

pub fn create_label(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Label::new(text.to_string(), geometry))
}

pub fn create_check_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut check_box = CheckBox::new(geometry);
    if !text.is_empty() {
        check_box.set_text(text.to_string());
    }
    Box::new(check_box)
}

pub fn create_radio_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut radio_button = RadioButton::new(geometry);
    if !text.is_empty() {
        radio_button.set_text(text.to_string());
    }
    Box::new(radio_button)
}

pub fn create_slider(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Slider::new(geometry))
}

pub fn create_progress_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ProgressBar::new(geometry))
}

pub fn create_scroll_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ScrollBar::new(geometry))
}

pub fn create_list_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ListBox::new(geometry))
}

pub fn create_spin_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SpinBox::new(geometry))
}

pub fn create_combo_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ComboBox::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_dial(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Dial::new(geometry))
}

pub fn create_window(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let title = if text.is_empty() { "Window".to_string() } else { text.to_string() };
    Box::new(Window::new(title, geometry))
}

pub fn create_group_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut group_box = GroupBox::new(geometry);
    if !text.is_empty() {
        group_box.set_title(text.to_string());
    }
    Box::new(group_box)
}

#[cfg(not(feature = "mini"))]
pub fn create_splitter(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Splitter::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_lcd_number(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(LCDNumber::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_command_link(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut command_link = CommandLink::new(geometry);
    if !text.is_empty() {
        command_link.set_text(text.to_string());
    }
    Box::new(command_link)
}

#[cfg(not(feature = "mini"))]
pub fn create_font_combo_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FontComboBox::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_action(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Action::new(text.to_string(), geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_tool_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ToolBox::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_tab_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TabBar::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_calendar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Calendar::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_date_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DateEdit::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_time_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TimeEdit::new(geometry))
}

pub fn create_line_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut line_edit = LineEdit::new(geometry);
    if !text.is_empty() {
        line_edit.set_text(text.to_string());
    }
    Box::new(line_edit)
}

#[cfg(not(feature = "mini"))]
pub fn create_list_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ListView::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_tree_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TreeView::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_table_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TableWidget::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_data_grid(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DataGrid::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_tree_table(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TreeTable::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_virtual_table(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(VirtualTable::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_virtual_list(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(VirtualList::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_menu(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Menu::new(text, geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_menu_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MenuBar::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_tool_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ToolBar::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_ribbon_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(RibbonBar::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_color_picker(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ColorPicker::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_code_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut editor = CodeEditor::new(geometry);
    if !text.is_empty() {
        editor.set_text(text.to_string());
    }
    Box::new(editor)
}

#[cfg(not(feature = "mini"))]
pub fn create_gantt_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(GanttWidget::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_terminal_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut terminal = TerminalView::new(geometry);
    if !text.is_empty() {
        terminal.set_input_line(text.to_string());
    }
    Box::new(terminal)
}

#[cfg(not(feature = "mini"))]
pub fn create_snackbar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut snackbar = Snackbar::new(geometry);
    if !text.is_empty() {
        snackbar.show(text.to_string());
    }
    Box::new(snackbar)
}

#[cfg(not(feature = "mini"))]
pub fn create_map_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MapView::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_media_player(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MediaPlayer::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_breadcrumb(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Breadcrumb::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_split_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(SplitButton::new(text.to_string(), geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_segmented_control(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SegmentedControl::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_chip(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Chip::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_grid(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(GridWidget::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_freeform_shape(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FreeformShapeWidget::new(geometry, ShapePath::RoundedRect { radius: 8 }))
}

// ── Dialog widget constructors ────────────────────────────────

#[cfg(not(feature = "mini"))]
pub fn create_message_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MessageBox::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_file_dialog(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FileDialog::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_font_dialog(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FontDialog::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_input_dialog(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(InputDialog::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_progress_dialog(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ProgressDialog::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_popup_window(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(PopupWindow::new(geometry))
}

// ── Container widget constructors ─────────────────────────────

pub fn create_scroll_area(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ScrollArea::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_tab_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TabWidget::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_stacked_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(StackedWidget::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_collapsible_pane(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(CollapsiblePane::new(geometry, String::new()))
}

#[cfg(not(feature = "mini"))]
pub fn create_dock_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DockWidget::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_mdi_area(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MdiArea::new(geometry))
}

// ── Text widget constructors ──────────────────────────────────

#[cfg(not(feature = "mini"))]
pub fn create_text_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TextEdit::new(geometry))
}

// ── Web widget constructors ───────────────────────────────────

#[cfg(not(feature = "mini"))]
pub fn create_web_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(WebView::new(geometry))
}

// ── Advanced widget constructors ──────────────────────────────

#[cfg(not(feature = "mini"))]
pub fn create_pie_menu(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(PieMenu::new(
        Point::new(
            geometry.x + (geometry.width / 2) as i32,
            geometry.y + (geometry.height / 2) as i32,
        ),
        geometry.width.min(geometry.height) as f32 / 2.0,
    ))
}

#[cfg(not(feature = "mini"))]
pub fn create_date_time_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DateTimeEdit::new(geometry))
}
