// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

#[cfg(full_widgets)]
use crate::core::Color;
#[cfg(full_widgets)]
use crate::core::Point;
use crate::core::Rect;
#[cfg(full_widgets)]
use crate::widget::special_widgets::freeform_shape::ShapePath;
use crate::widget::*;
use std::boxed::Box;

/// Applies a caller-supplied label to a freshly built widget.
///
/// # Why this exists
///
/// 136 of the 155 constructors in this file take `_text` and discard it, so a
/// caller that passed a title got a control with no text at all — a real defect
/// masked by the fact that the old platform path stored the string separately.
/// Now that the widget owns its state, the label has to reach the widget.
///
/// The write goes through the property contract rather than each type's own setter,
/// because the property name differs (`text` for a button, `title` for a window or
/// group box). Going through the contract keeps this helper independent of the
/// concrete type, and a type that exposes neither name is simply left alone — the
/// label is genuinely not part of that control's contract.
fn label(geometry: Rect, text: &str, widget: Box<dyn Widget>) -> Box<dyn Widget> {
    if text.is_empty() {
        return widget;
    }
    let mut widget = widget;
    let value = crate::widget::capability::CapabilityValue::String(text.to_string());
    // Whichever spelling the control publishes wins; none being known means the
    // control has no label concept, which is not an error.
    for property in crate::control_backend::custom::LABEL_PROPERTY_NAMES {
        if crate::widget::capability::widget_property_set(widget.as_mut(), property, value.clone())
            .is_ok()
        {
            return widget;
        }
    }
    let _ = geometry;
    widget
}

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

pub fn create_slider(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Slider::new(geometry)))
}

pub fn create_progress_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ProgressBar::new(geometry)))
}

pub fn create_scroll_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ScrollBar::new(geometry)))
}

pub fn create_list_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ListBox::new(geometry)))
}

pub fn create_spin_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SpinBox::new(geometry)))
}

pub fn create_combo_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ComboBox::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_dial(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Dial::new(geometry)))
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

#[cfg(full_widgets)]
pub fn create_splitter(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Splitter::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_lcd_number(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(LCDNumber::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_command_link(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut command_link = CommandLink::new(geometry);
    if !text.is_empty() {
        command_link.set_text(text.to_string());
    }
    Box::new(command_link)
}

#[cfg(full_widgets)]
pub fn create_font_combo_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FontComboBox::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_action(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Action::new(text.to_string(), geometry))
}

#[cfg(full_widgets)]
pub fn create_tool_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ToolBox::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_tab_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TabBar::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_calendar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Calendar::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_date_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DateEdit::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_time_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TimeEdit::new(geometry)))
}

pub fn create_line_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut line_edit = LineEdit::new(geometry);
    if !text.is_empty() {
        line_edit.set_text(text.to_string());
    }
    Box::new(line_edit)
}

#[cfg(full_widgets)]
pub fn create_list_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ListView::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_tree_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TreeView::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_table_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TableWidget::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_data_grid(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DataGrid::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_tree_table(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TreeTable::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_virtual_table(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(VirtualTable::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_virtual_list(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(VirtualList::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_menu(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Menu::new(text, geometry))
}

#[cfg(full_widgets)]
pub fn create_menu_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MenuBar::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_tool_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ToolBar::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_ribbon_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(RibbonBar::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_color_picker(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ColorPicker::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_code_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut editor = CodeEditor::new(geometry);
    if !text.is_empty() {
        editor.set_text(text.to_string());
    }
    Box::new(editor)
}

#[cfg(full_widgets)]
pub fn create_gantt_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(GanttWidget::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_terminal_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut terminal = TerminalView::new(geometry);
    if !text.is_empty() {
        terminal.set_input_line(text.to_string());
    }
    Box::new(terminal)
}

#[cfg(full_widgets)]
pub fn create_snackbar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut snackbar = Snackbar::new(geometry);
    if !text.is_empty() {
        snackbar.show(text.to_string());
    }
    Box::new(snackbar)
}

#[cfg(full_widgets)]
pub fn create_map_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MapView::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_media_player(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MediaPlayer::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_breadcrumb(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Breadcrumb::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_split_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(SplitButton::new(text.to_string(), geometry))
}

#[cfg(full_widgets)]
pub fn create_segmented_control(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SegmentedControl::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_chip(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Chip::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_grid(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(GridWidget::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_freeform_shape(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(
        geometry,
        text,
        Box::new(FreeformShapeWidget::new(geometry, ShapePath::RoundedRect { radius: 8 })),
    )
}

// ── Always-available widget constructors (not gated by mini) ───

#[cfg(full_widgets)]
pub fn create_toggle_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut btn = ToggleButton::new(text.to_string(), geometry);
    if !text.is_empty() {
        btn.set_text(text.to_string());
    }
    Box::new(btn)
}

pub fn create_arc(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Arc::new(geometry)))
}

pub fn create_spinner(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Spinner::new(geometry)))
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

pub fn create_keyboard(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Keyboard::new(geometry)))
}

pub fn create_switch(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Switch::new(geometry)))
}

pub fn create_line(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(
        geometry,
        text,
        Box::new(Line::new(
            crate::widget::display_widgets::line::LineOrientation::Horizontal,
            geometry,
        )),
    )
}

pub fn create_meter(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Meter::new(geometry)))
}

pub fn create_mini_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MiniChart::new(geometry)))
}

pub fn create_image_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ImageView::new(crate::widget::Image::new(), geometry)))
}

pub fn create_mini_canvas(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MiniCanvas::new(geometry)))
}

pub fn create_tile_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TileView::new(geometry)))
}

// ── Dialog widget constructors ────────────────────────────────

#[cfg(full_widgets)]
pub fn create_message_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MessageBox::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_file_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FileDialog::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_font_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FontDialog::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_input_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(InputDialog::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_progress_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ProgressDialog::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_popup_window(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(PopupWindow::new(geometry)))
}

// ── Container widget constructors ─────────────────────────────

pub fn create_scroll_area(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ScrollArea::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_tab_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TabWidget::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_stacked_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(StackedWidget::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_collapsible_pane(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CollapsiblePane::new(geometry, String::new())))
}

#[cfg(full_widgets)]
pub fn create_dock_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DockWidget::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_mdi_area(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MdiArea::new(geometry)))
}

// ── Text widget constructors ──────────────────────────────────

#[cfg(full_widgets)]
pub fn create_text_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TextEdit::new(geometry)))
}

// ── Web widget constructors ───────────────────────────────────

#[cfg(full_widgets)]
pub fn create_web_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    // WebView is now an alias for WebEngineView.
    Box::new(WebEngineView::new(geometry))
}

// ── Advanced widget constructors ──────────────────────────────

#[cfg(full_widgets)]
pub fn create_pie_menu(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(
        geometry,
        text,
        Box::new(PieMenu::new(
            Point::new(
                geometry.x + (geometry.width / 2) as i32,
                geometry.y + (geometry.height / 2) as i32,
            ),
            geometry.width.min(geometry.height) as f32 / 2.0,
        )),
    )
}

#[cfg(full_widgets)]
pub fn create_date_time_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DateTimeEdit::new(geometry)))
}

// ── Group A widget constructors (non-mini) ─────────────────────

#[cfg(full_widgets)]
pub fn create_canvas(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Canvas::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ChartWidget::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_search_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SearchBox::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_badge(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Badge::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_skeleton_loader(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SkeletonLoader::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_fab(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FAB::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_bottom_sheet(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(BottomSheet::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_bottom_navigation_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(BottomNavigationBar::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_navigation_drawer(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(NavigationDrawer::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_app_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(AppBar::new("", geometry)))
}

#[cfg(full_widgets)]
pub fn create_mobile_date_picker(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MobileDatePicker::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_divider(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Divider::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_stepper(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Stepper::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_rating(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Rating::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_avatar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Avatar::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_empty_state(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(EmptyState::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_color_history(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ColorHistory::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_color_well(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ColorWell::new(Color::rgba(255, 0, 0, 255), geometry)))
}

#[cfg(full_widgets)]
pub fn create_tag_input(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TagInput::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_ime_preedit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ImePreedit::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_inplace_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(InplaceEditor::new("", geometry)))
}

#[cfg(full_widgets)]
pub fn create_qr_code(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(QRCode::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_masonry_layout(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MasonryLayout::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_material_snackbar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MaterialSnackbar::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_adaptive_scaffold(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(AdaptiveScaffold::new("", geometry)))
}

#[cfg(full_widgets)]
pub fn create_wizard_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(WizardDialog::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_safe_area(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SafeArea::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_cupertino_alert_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CupertinoAlertDialog::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_cupertino_slider(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CupertinoSlider::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_tooltip(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Tooltip::new("", geometry)))
}

#[cfg(full_widgets)]
pub fn create_segmented_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SegmentedButton::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_navigation_stack(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(NavigationStack::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_progress_circle(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ProgressCircle::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_icon(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Icon::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_dropdown_menu(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DropdownMenu::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_masked_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MaskedEdit::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_menu_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MenuButton::new("", geometry)))
}

#[cfg(full_widgets)]
pub fn create_popover(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Popover::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_auto_complete_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(AutoCompleteEdit::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_multi_select_combo_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MultiSelectComboBox::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_range_slider(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(RangeSlider::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_floating_label(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FloatingLabel::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_font_preview(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FontPreview::new("Arial", geometry)))
}

#[cfg(full_widgets)]
pub fn create_cupertino_navigation_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CupertinoNavigationBar::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_cupertino_segmented_control(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CupertinoSegmentedControl::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_refresh_control(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(RefreshControl::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_modal_bottom_sheet(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ModalBottomSheet::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_find_replace_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FindReplaceDialog::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_properties_panel(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(PropertiesPanel::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_cupertino_date_picker(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CupertinoDatePicker::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_editable_combo_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(EditableComboBox::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_date_range_picker(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DateRangePicker::new(geometry)))
}

// ── New widget constructors (non-mini) ───────────────────────────

#[cfg(full_widgets)]
pub fn create_rich_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(RichEdit::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_carousel(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Carousel::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_material_navigation_rail(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MaterialNavigationRail::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_tab_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TabView::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_search_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SearchBar::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_shortcut_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ShortcutEditor::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_swipe_to_dismiss(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SwipeToDismiss::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_pager_page_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(PagerPageView::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_line_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(LineChart::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_sparkline(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Sparkline::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_bar_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(BarChart::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_pie_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(PieChart::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_animated_image(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(AnimatedImage::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_hero_animation(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(HeroAnimation::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_bezier_curve_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(BezierCurveEditor::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_lottie_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(LottieWidget::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_rive_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(RiveWidget::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_video_player(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(VideoPlayer::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_image_gallery(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ImageGallery::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_audio_visualizer(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(AudioVisualizer::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_camera_preview(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CameraPreview::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_barcode_scanner(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(BarcodeScanner::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_tool_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(ToolButton::new(text.to_string(), geometry))
}

#[cfg(full_widgets)]
pub fn create_status_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(StatusBar::new(geometry)))
}

#[cfg(full_widgets)]
pub fn create_property_grid(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(PropertyGrid::new(geometry)))
}
