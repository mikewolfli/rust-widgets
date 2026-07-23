#[cfg(not(feature = "mini"))]
use crate::core::Color;
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

#[cfg(not(any(feature = "mini", feature = "embedded")))]
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

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_tab_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TabBar::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_calendar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Calendar::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_date_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DateEdit::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
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

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_ribbon_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(RibbonBar::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_color_picker(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ColorPicker::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_code_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut editor = CodeEditor::new(geometry);
    if !text.is_empty() {
        editor.set_text(text.to_string());
    }
    Box::new(editor)
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_gantt_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(GanttWidget::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_terminal_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut terminal = TerminalView::new(geometry);
    if !text.is_empty() {
        terminal.set_input_line(text.to_string());
    }
    Box::new(terminal)
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_snackbar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut snackbar = Snackbar::new(geometry);
    if !text.is_empty() {
        snackbar.show(text.to_string());
    }
    Box::new(snackbar)
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_map_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MapView::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_media_player(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MediaPlayer::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_breadcrumb(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Breadcrumb::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_split_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(SplitButton::new(text.to_string(), geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_segmented_control(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SegmentedControl::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_chip(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Chip::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_grid(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(GridWidget::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_freeform_shape(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FreeformShapeWidget::new(geometry, ShapePath::RoundedRect { radius: 8 }))
}

// ── Always-available widget constructors (not gated by mini) ───

#[cfg(not(feature = "mini"))]
pub fn create_toggle_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut btn = ToggleButton::new(text.to_string(), geometry);
    if !text.is_empty() {
        btn.set_text(text.to_string());
    }
    Box::new(btn)
}

pub fn create_arc(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Arc::new(geometry))
}

pub fn create_spinner(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Spinner::new(geometry))
}

pub fn create_roller(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let options = if text.is_empty() {
        vec!["Option 1".to_string(), "Option 2".to_string(), "Option 3".to_string()]
    } else {
        text.lines().map(|s| s.to_string()).collect()
    };
    Box::new(Roller::new(options, geometry))
}

pub fn create_dropdown(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let items =
        if text.is_empty() { Vec::new() } else { text.lines().map(|s| s.to_string()).collect() };
    Box::new(Dropdown::new(items, geometry))
}

pub fn create_textarea(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(TextArea::new(text.to_string(), geometry))
}

pub fn create_keyboard(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Keyboard::new(geometry))
}

pub fn create_switch(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Switch::new(geometry))
}

pub fn create_line(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Line::new(crate::widget::display_widgets::line::LineOrientation::Horizontal, geometry))
}

pub fn create_meter(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Meter::new(geometry))
}

pub fn create_mini_chart(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MiniChart::new(geometry))
}

pub fn create_image_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ImageView::new(crate::widget::Image::new(), geometry))
}

pub fn create_mini_canvas(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MiniCanvas::new(geometry))
}

pub fn create_tile_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TileView::new(geometry))
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
    // WebView is now an alias for WebEngineView.
    Box::new(WebEngineView::new(geometry))
}

// ── Advanced widget constructors ──────────────────────────────

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_pie_menu(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(PieMenu::new(
        Point::new(
            geometry.x + (geometry.width / 2) as i32,
            geometry.y + (geometry.height / 2) as i32,
        ),
        geometry.width.min(geometry.height) as f32 / 2.0,
    ))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_date_time_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DateTimeEdit::new(geometry))
}

// ── Group A widget constructors (non-mini) ─────────────────────

#[cfg(not(feature = "mini"))]
pub fn create_canvas(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Canvas::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_chart(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ChartWidget::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_search_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SearchBox::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_badge(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Badge::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_skeleton_loader(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SkeletonLoader::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_fab(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FAB::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_bottom_sheet(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(BottomSheet::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_bottom_navigation_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(BottomNavigationBar::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_navigation_drawer(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(NavigationDrawer::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_app_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(AppBar::new("", geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_mobile_date_picker(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MobileDatePicker::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_divider(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Divider::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_stepper(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Stepper::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_rating(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Rating::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_avatar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Avatar::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_empty_state(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(EmptyState::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_color_history(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ColorHistory::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_color_well(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ColorWell::new(Color::rgba(255, 0, 0, 255), geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_tag_input(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TagInput::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_ime_preedit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ImePreedit::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_inplace_editor(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(InplaceEditor::new("", geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_qr_code(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(QRCode::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_masonry_layout(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MasonryLayout::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_material_snackbar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MaterialSnackbar::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_adaptive_scaffold(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(AdaptiveScaffold::new("", geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_wizard_dialog(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(WizardDialog::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_safe_area(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SafeArea::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_cupertino_alert_dialog(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(CupertinoAlertDialog::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_cupertino_slider(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(CupertinoSlider::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_tooltip(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Tooltip::new("", geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_segmented_button(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SegmentedButton::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_navigation_stack(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(NavigationStack::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_progress_circle(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ProgressCircle::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_icon(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Icon::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_dropdown_menu(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DropdownMenu::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_masked_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MaskedEdit::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_menu_button(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MenuButton::new("", geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_popover(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Popover::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_auto_complete_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(AutoCompleteEdit::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_multi_select_combo_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MultiSelectComboBox::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_range_slider(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(RangeSlider::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_floating_label(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FloatingLabel::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_font_preview(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FontPreview::new("Arial", geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_cupertino_navigation_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(CupertinoNavigationBar::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_cupertino_segmented_control(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(CupertinoSegmentedControl::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_refresh_control(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(RefreshControl::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_modal_bottom_sheet(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ModalBottomSheet::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_find_replace_dialog(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FindReplaceDialog::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_properties_panel(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(PropertiesPanel::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_cupertino_date_picker(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(CupertinoDatePicker::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_editable_combo_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(EditableComboBox::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_date_range_picker(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DateRangePicker::new(geometry))
}

// ── New widget constructors (non-mini) ───────────────────────────

#[cfg(not(feature = "mini"))]
pub fn create_rich_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(RichEdit::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_carousel(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Carousel::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_material_navigation_rail(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MaterialNavigationRail::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_tab_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TabView::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_search_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SearchBar::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_shortcut_editor(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ShortcutEditor::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_swipe_to_dismiss(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SwipeToDismiss::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_pager_page_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(PagerPageView::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_line_chart(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(LineChart::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_sparkline(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Sparkline::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_bar_chart(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(BarChart::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_pie_chart(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(PieChart::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_animated_image(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(AnimatedImage::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_hero_animation(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(HeroAnimation::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_bezier_curve_editor(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(BezierCurveEditor::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_lottie_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(LottieWidget::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_rive_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(RiveWidget::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_video_player(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(VideoPlayer::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_image_gallery(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ImageGallery::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_audio_visualizer(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(AudioVisualizer::new(geometry))
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn create_camera_preview(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(CameraPreview::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_barcode_scanner(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(BarcodeScanner::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_tool_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(ToolButton::new(text.to_string(), geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_status_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(StatusBar::new(geometry))
}

#[cfg(not(feature = "mini"))]
pub fn create_property_grid(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(PropertyGrid::new(geometry))
}
