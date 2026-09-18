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

/// Creates a push button. `text` becomes the button's label.
pub fn create_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Button::new(text.to_string(), geometry))
}

/// Creates a static text label showing `text`.
pub fn create_label(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Label::new(text.to_string(), geometry))
}

/// Creates a check box labelled `text`. An empty `text` leaves the caption
/// empty rather than falling back to a default.
pub fn create_check_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut check_box = CheckBox::new(geometry);
    if !text.is_empty() {
        check_box.set_text(text.to_string());
    }
    Box::new(check_box)
}

/// Creates a radio button labelled `text`. An empty `text` leaves the caption
/// empty rather than falling back to a default.
pub fn create_radio_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut radio_button = RadioButton::new(geometry);
    if !text.is_empty() {
        radio_button.set_text(text.to_string());
    }
    Box::new(radio_button)
}

/// Creates a horizontal slider with the track's default range. `text` is applied
/// as the control's label.
pub fn create_slider(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Slider::new(geometry)))
}

/// Creates a progress bar with the control's default range. `text` is applied as
/// the control's label.
pub fn create_progress_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ProgressBar::new(geometry)))
}

/// Creates a scroll bar oriented along the longer side of `geometry`. `text` is
/// applied as the control's label.
pub fn create_scroll_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ScrollBar::new(geometry)))
}

/// Creates an empty list box. `text` is applied as the control's label.
pub fn create_list_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ListBox::new(geometry)))
}

/// Creates a spin box with the control's default range. `text` is applied as the
/// control's label.
pub fn create_spin_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SpinBox::new(geometry)))
}

/// Creates a combo box with no items. `text` is applied as the control's label.
pub fn create_combo_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ComboBox::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a rotary dial with the control's default range. `text` is applied as
/// the control's label.
pub fn create_dial(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Dial::new(geometry)))
}

/// Creates a top-level window whose title is `text`, or `"Window"` when `text`
/// is empty.
pub fn create_window(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let title = if text.is_empty() { "Window".to_string() } else { text.to_string() };
    Box::new(Window::new(title, geometry))
}

/// Creates a group box with `text` as its title. An empty `text` leaves the
/// title empty rather than falling back to a default.
pub fn create_group_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut group_box = GroupBox::new(geometry);
    if !text.is_empty() {
        group_box.set_title(text.to_string());
    }
    Box::new(group_box)
}

#[cfg(full_widgets)]
/// Creates a horizontal splitter with no child panes. `text` is applied as the
/// control's label.
pub fn create_splitter(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Splitter::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a boxed frame that draws a border around its child. `text` is applied
/// as the control's label.
pub fn create_frame(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Frame::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a seven-segment number display reading zero. `text` is applied as the
/// control's label.
pub fn create_lcd_number(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(LCDNumber::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a command link button labelled `text`. An empty `text` leaves the
/// label empty rather than falling back to a default.
pub fn create_command_link(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut command_link = CommandLink::new(geometry);
    if !text.is_empty() {
        command_link.set_text(text.to_string());
    }
    Box::new(command_link)
}

#[cfg(full_widgets)]
/// Creates a combo box for choosing a font. `text` is applied as the control's
/// label.
pub fn create_font_combo_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FontComboBox::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a menu- or tool-bar action whose display name is `text`.
pub fn create_action(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Action::new(text.to_string(), geometry))
}

#[cfg(full_widgets)]
/// Creates a tool box with no pages. `text` is applied as the control's label.
pub fn create_tool_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ToolBox::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a toolbox with no items. `text` is applied as the control's label.
///
/// The same `ToolBox` control as [`create_tool_box`] under the spelling the
/// `toolbox` factory name uses. Both names have to exist because the factory
/// resolves the `toolbox` alias to its own constructor — a control table row is
/// `(capability, constructor)`, so an alias without a constructor cannot be
/// registered at all.
pub fn create_toolbox(geometry: Rect, text: &str) -> Box<dyn Widget> {
    create_tool_box(geometry, text)
}

#[cfg(full_widgets)]
/// Creates a tab bar with no tabs. `text` is applied as the control's label.
pub fn create_tab_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TabBar::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a calendar showing the current month. `text` is applied as the
/// control's label.
pub fn create_calendar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Calendar::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a date editor initialised to the current date. `text` is applied as
/// the control's label.
pub fn create_date_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DateEdit::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a time editor initialised to the current time. `text` is applied as
/// the control's label.
pub fn create_time_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TimeEdit::new(geometry)))
}

/// Creates a single-line text entry initialised to `text`.
pub fn create_line_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut line_edit = LineEdit::new(geometry);
    if !text.is_empty() {
        line_edit.set_text(text.to_string());
    }
    Box::new(line_edit)
}

#[cfg(full_widgets)]
/// Creates a list view with no rows or columns. `text` is applied as the
/// control's label.
pub fn create_list_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ListView::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a tree view with no items. `text` is applied as the control's label.
pub fn create_tree_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TreeView::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a table widget with no rows or columns. `text` is applied as the
/// control's label.
pub fn create_table_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TableWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates the plain `Table` control with no rows or columns.
///
/// Distinct from [`create_table_widget`]: `Table` reports `WidgetKind::Table` and
/// publishes the `table` factory name, which the C ABI's `create_table` addresses.
/// Before this constructor existed the only way to reach a table by name was the
/// `table_widget` entry, so `WidgetKind::Table` had no capability *named after it*
/// and the kind lookup resolved to whichever table-like entry registered first.
pub fn create_table(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TableWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a data grid with no rows or columns. `text` is applied as the
/// control's label.
pub fn create_data_grid(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DataGrid::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a tree table with no rows or columns. `text` is applied as the
/// control's label.
pub fn create_tree_table(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TreeTable::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a table that materialises only the visible rows. `text` is applied as
/// the control's label.
pub fn create_virtual_table(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(VirtualTable::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a list that materialises only the visible rows. `text` is applied as
/// the control's label.
pub fn create_virtual_list(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(VirtualList::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a data view with no data source. `text` is applied as the control's label.
///
/// The same `VirtualList` control as [`create_virtual_list`] under the `DataView`
/// spelling — `widget::mod.rs` declares `pub type DataView = VirtualList;`, and a kind
/// whose constructor cannot be named is a kind `create_data_view` cannot build.
pub fn create_data_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    create_virtual_list(geometry, text)
}

#[cfg(full_widgets)]
/// Creates a menu with no items whose title is `text`.
pub fn create_menu(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Menu::new(text, geometry))
}

#[cfg(full_widgets)]
/// Creates a menu bar with no menus. `text` is applied as the control's label.
pub fn create_menu_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MenuBar::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a tool bar with no actions. `text` is applied as the control's label.
pub fn create_tool_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ToolBar::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a ribbon bar with no tabs. `text` is applied as the control's label.
pub fn create_ribbon_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(RibbonBar::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a colour picker with the control's default colour. `text` is applied
/// as the control's label.
pub fn create_color_picker(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ColorPicker::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a modal colour dialog. `text` is applied as the control's label.
///
/// Distinct from [`create_color_picker`]: the dialog is the window that hosts a
/// picker and carries accept/reject, so it is a different control rather than a
/// differently-named picker.
pub fn create_color_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ColorDialog::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a panel container with no children and no label.
///
/// `Panel` is a type alias for `GroupBox` (`src/widget/mod.rs`), so this is the same
/// control `create_group_box(..)` builds, under the `panel` factory name. The second
/// name is not redundant: the capability the `panel` name is registered against is
/// what `create_panel(..)` mounts, and without it `WidgetKind::Panel` had no
/// capability *named after it*, so the kind lookup fell through to `breadcrumb` — a
/// navigation trail where the caller asked for an empty container.
pub fn create_panel(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(GroupBox::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a code editor with no document loaded. `text` becomes the editor's
/// initial contents.
pub fn create_code_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut editor = CodeEditor::new(geometry);
    if !text.is_empty() {
        editor.set_text(text.to_string());
    }
    Box::new(editor)
}

#[cfg(full_widgets)]
/// Creates a Gantt chart with no tasks. `text` is applied as the control's
/// label.
pub fn create_gantt_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(GanttWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a terminal view with no output. `text` becomes the initial contents of
/// the input line, not the transcript.
pub fn create_terminal_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut terminal = TerminalView::new(geometry);
    if !text.is_empty() {
        terminal.set_input_line(text.to_string());
    }
    Box::new(terminal)
}

#[cfg(full_widgets)]
/// Creates a snackbar in the dismissed state. When `text` is non-empty the
/// snackbar is shown immediately with that message.
pub fn create_snackbar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut snackbar = Snackbar::new(geometry);
    if !text.is_empty() {
        snackbar.show(text.to_string());
    }
    Box::new(snackbar)
}

#[cfg(full_widgets)]
/// Creates a map view with no tiles or markers. `text` is applied as the
/// control's label.
pub fn create_map_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MapView::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a media player with no source loaded. `text` is applied as the
/// control's label.
pub fn create_media_player(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MediaPlayer::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a breadcrumb trail with no segments. `text` is applied as the
/// control's label.
pub fn create_breadcrumb(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Breadcrumb::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an empty signature pad. `text` is applied as the control's label.
pub fn create_signature_pad(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SignaturePad::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a drop zone that accepts `text/plain`. `text` is applied as the
/// control's label; the accepted MIME type is set via the `accepted_type`
/// property or [`DropZone::set_accepted_type`].
pub fn create_drop_zone(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DropZone::new(geometry, "text/plain")))
}

#[cfg(full_widgets)]
/// Creates a split button whose primary label is `text`.
pub fn create_split_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(SplitButton::new(text.to_string(), geometry))
}

#[cfg(full_widgets)]
/// Creates a segmented control with no segments. `text` is applied as the
/// control's label.
pub fn create_segmented_control(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SegmentedControl::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a chip with no label of its own. `text` is applied as the control's
/// label.
pub fn create_chip(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Chip::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a grid layout with no child cells. `text` is applied as the control's
/// label.
pub fn create_grid(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(GridWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a timeline with no items. `text` is applied as the control's label.
pub fn create_timeline_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TimelineWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a command palette with no entries. `text` is applied as the control's
/// label.
pub fn create_command_palette(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CommandPalette::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a notification center with no notifications. `text` is applied as the
/// control's label.
pub fn create_notification_center(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(NotificationCenter::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a diff viewer with both snapshots empty. `text` is applied as the
/// control's label.
pub fn create_diff_viewer(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DiffViewer::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an empty markdown editor. `text` is applied as the control's label.
pub fn create_markdown_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MarkdownEditor::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an empty toast stack. `text` is applied as the control's label.
pub fn create_toast_stack(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ToastStack::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a grid table with no data source attached. `text` is applied as the
/// control's label.
pub fn create_grid_table(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(GridTableWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a number picker over `0..=100`. `text` is applied as the control's
/// label.
pub fn create_number_picker(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(NumberPicker::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a six-box OTP input. `text` seeds the code via the `value` property.
pub fn create_otp_input(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(OtpInput::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an informational banner with an empty message.
pub fn create_banner(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Banner::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a pagination bar over no items. `text` is applied as the control's
/// label.
pub fn create_pagination(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Pagination::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a single toast showing `text` at informational level.
pub fn create_toast(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Toast::new(geometry, text)))
}

#[cfg(full_widgets)]
/// Creates a splash screen titled `text` with indeterminate progress.
pub fn create_splash_screen(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SplashScreen::new(geometry, text)))
}

#[cfg(full_widgets)]
/// Creates a free-form shape widget drawn as a rounded rectangle with a corner
/// radius of 8 pixels. `text` is applied as the control's label.
pub fn create_freeform_shape(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(
        geometry,
        text,
        Box::new(FreeformShapeWidget::new(geometry, ShapePath::RoundedRect { radius: 8 })),
    )
}

// ── Always-available widget constructors (not gated by mini) ───

#[cfg(full_widgets)]
/// Creates a toggle button labelled `text`. An empty `text` leaves the label
/// empty rather than falling back to a default.
pub fn create_toggle_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut btn = ToggleButton::new(text.to_string(), geometry);
    if !text.is_empty() {
        btn.set_text(text.to_string());
    }
    Box::new(btn)
}

/// Creates an arc with the control's default sweep. `text` is applied as the
/// control's label.
pub fn create_arc(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Arc::new(geometry)))
}

/// Creates a spinner in its stopped state. `text` is applied as the control's
/// label.
pub fn create_spinner(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Spinner::new(geometry)))
}

/// Creates a roller selection wheel. Each line of `text` becomes one option; an
/// empty `text` yields the three placeholder options `Option 1` to `Option 3`.
pub fn create_roller(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let options = if text.is_empty() {
        vec!["Option 1".to_string(), "Option 2".to_string(), "Option 3".to_string()]
    } else {
        text.lines().map(|s| s.to_string()).collect()
    };
    Box::new(Roller::new(options, geometry))
}

/// Creates a drop-down list. Each line of `text` becomes one item; an empty
/// `text` leaves the list empty.
pub fn create_dropdown(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let items =
        if text.is_empty() { Vec::new() } else { text.lines().map(|s| s.to_string()).collect() };
    Box::new(Dropdown::new(items, geometry))
}

/// Creates a multi-line text area initialised to `text`.
pub fn create_textarea(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(TextArea::new(text.to_string(), geometry))
}

/// Creates an on-screen keyboard with the platform's default layout. `text` is
/// applied as the control's label.
pub fn create_keyboard(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Keyboard::new(geometry)))
}

/// Creates a switch in the off position. `text` is applied as the control's
/// label.
pub fn create_switch(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Switch::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a Cupertino (iOS-styled) switch in the off position. `text` is applied
/// as the control's label.
pub fn create_cupertino_switch(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CupertinoSwitch::new(geometry)))
}

/// Creates a horizontal separator line. `text` is applied as the control's
/// label.
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

/// Creates a meter gauge with the control's default range. `text` is applied as
/// the control's label.
pub fn create_meter(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Meter::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a radar chart with no axes and no series. `text` is applied as the
/// control's label.
///
/// Empty rather than pre-populated with sample dimensions: an axis name is the
/// caller's domain vocabulary, and a default like `"Axis 1"` would have to be
/// deleted before the control was usable.
pub fn create_radar_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(RadarChart::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an empty kanban board. `text` is applied as the control's label.
///
/// No sample columns: a column title is the caller's workflow vocabulary, and a
/// default like `"Column 1"` would have to be deleted before the board was usable.
pub fn create_kanban_board(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(KanbanBoard::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an empty cascader. `text` is applied as the control's label.
///
/// No sample options: the tree is the caller's domain vocabulary, and a default
/// like `"Level 1"` would have to be deleted before the control was usable.
pub fn create_cascader(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Cascader::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an empty query builder. `text` is applied as the control's label.
///
/// No default fields: a filter field is the caller's schema vocabulary, and a
/// default like `"Column 1"` would have to be replaced before the builder was
/// usable.
pub fn create_query_builder(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(QueryBuilder::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an empty emoji picker. `text` is applied as the control's label.
///
/// No glyph table: the shell is deliberately data-free, and the caller supplies the
/// symbols through `set_glyphs`.
pub fn create_emoji_picker(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(EmojiPicker::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an empty mention field. `text` is applied as the control's label.
///
/// No default candidates: a mention candidate is the caller's people list, and any
/// placeholder would have to be replaced before the control was usable.
pub fn create_mention(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Mention::new(geometry)))
}

/// Creates a small inline chart with no data. `text` is applied as the control's
/// label.
pub fn create_mini_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MiniChart::new(geometry)))
}

/// Creates an image view with no image loaded. `text` is applied as the
/// control's label.
pub fn create_image_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ImageView::new(crate::widget::Image::new(), geometry)))
}

/// Creates a small drawing surface with an empty scene. `text` is applied as the
/// control's label.
pub fn create_mini_canvas(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MiniCanvas::new(geometry)))
}

// ── Dialog widget constructors ────────────────────────────────

#[cfg(full_widgets)]
/// Creates a message box in its default state: no icon, an `OK` button and no
/// title or body text. `text` becomes the box's text, not its title.
pub fn create_message_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MessageBox::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a file dialog with no filter, starting directory or selection.
/// `text` is applied as the control's label.
pub fn create_file_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FileDialog::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a font selection dialog using the platform's default selection.
/// `text` is applied as the control's label.
pub fn create_font_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FontDialog::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a single-line input dialog with no prompt and an empty field. `text`
/// is applied as the control's label.
pub fn create_input_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(InputDialog::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a modal progress dialog with no cancellation support. `text` is
/// applied as the control's label.
pub fn create_progress_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ProgressDialog::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a popup window with no content. `text` is applied as the control's
/// label.
pub fn create_popup_window(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(PopupWindow::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a generic titled dialog with no content. `text` is applied as the
/// control's title.
///
/// Distinct from [`create_popup_window`] (a chrome-only popup) and from
/// [`create_message_box`] (a fixed message plus buttons): this is the standard
/// secondary window a caller fills with its own form via `set_content_widget`.
pub fn create_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Dialog::with_title("", geometry)))
}

// ── Container widget constructors ─────────────────────────────

/// Creates a scrollable viewport with no content. `text` is applied as the
/// control's label.
pub fn create_scroll_area(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ScrollArea::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a tab widget with no pages. `text` is applied as the control's label.
pub fn create_tab_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TabWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a stacked widget with no pages. `text` is applied as the control's
/// label.
pub fn create_stacked_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(StackedWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a collapsible pane with an empty header title. `text` is applied as
/// the control's label.
pub fn create_collapsible_pane(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CollapsiblePane::new(geometry, String::new())))
}

#[cfg(full_widgets)]
/// Creates a dockable panel with no content. `text` is applied as the control's
/// label.
pub fn create_dock_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DockWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a multiple-document interface area with no child windows. `text` is
/// applied as the control's label.
pub fn create_mdi_area(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MdiArea::new(geometry)))
}

// ── Text widget constructors ──────────────────────────────────

#[cfg(full_widgets)]
/// Creates a rich text editor with no document loaded. `text` is applied as the
/// control's label.
pub fn create_text_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TextEdit::new(geometry)))
}

// ── Web widget constructors ───────────────────────────────────

#[cfg(full_widgets)]
/// Creates a web engine view with no page loaded.
///
/// `_text` is deliberately ignored: the underlying view has no label concept, so
/// there is nothing for it to configure.
pub fn create_web_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    // WebView is now an alias for WebEngineView.
    Box::new(WebEngineView::new(geometry))
}

// ── Advanced widget constructors ──────────────────────────────

#[cfg(full_widgets)]
/// Creates a pie menu centred on `geometry`, with a radius of half the shorter
/// side so that it fits inside the rectangle. `text` is applied as the control's
/// label.
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
/// Creates a combined date and time editor initialised to the current moment.
/// `text` is applied as the control's label.
pub fn create_date_time_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DateTimeEdit::new(geometry)))
}

// ── Group A widget constructors (non-mini) ─────────────────────

#[cfg(full_widgets)]
/// Creates a drawing surface with an empty scene. `text` is applied as the
/// control's label.
pub fn create_canvas(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Canvas::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a chart with no series. `text` is applied as the control's label.
pub fn create_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ChartWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a search box with an empty query. `text` is applied as the control's
/// label.
pub fn create_search_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SearchBox::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a badge with no count or text. `text` is applied as the control's
/// label.
pub fn create_badge(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Badge::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a placeholder skeleton shown while content loads. `text` is applied
/// as the control's label.
pub fn create_skeleton_loader(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SkeletonLoader::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a floating action button with no icon or label. `text` is applied as
/// the control's label.
pub fn create_fab(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FAB::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a bottom sheet in its collapsed state with no content. `text` is
/// applied as the control's label.
pub fn create_bottom_sheet(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(BottomSheet::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a bottom navigation bar with no items. `text` is applied as the
/// control's label.
pub fn create_bottom_navigation_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(BottomNavigationBar::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a navigation drawer with no items. `text` is applied as the control's
/// label.
pub fn create_navigation_drawer(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(NavigationDrawer::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an app bar with an empty title and no back or action affordances.
/// `text` is applied through the label contract, so it lands on the bar's
/// `/Title` rather than being passed to the constructor.
pub fn create_app_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(AppBar::new("", geometry)))
}

#[cfg(full_widgets)]
/// Creates a mobile date picker opened on the current date. `text` is applied as
/// the control's label.
pub fn create_mobile_date_picker(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MobileDatePicker::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a horizontal divider rule. `text` is applied as the control's label.
pub fn create_divider(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Divider::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a stepper at its first step with no steps defined. `text` is applied
/// as the control's label.
pub fn create_stepper(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Stepper::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a star rating control with no rating set. `text` is applied as the
/// control's label.
pub fn create_rating(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Rating::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an avatar with no image or initials. `text` is applied as the
/// control's label.
pub fn create_avatar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Avatar::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a placeholder shown when a collection has no items. `text` is applied
/// as the control's label.
pub fn create_empty_state(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(EmptyState::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a colour history strip recording no previously used colours. `text`
/// is applied as the control's label.
pub fn create_color_history(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ColorHistory::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a colour swatch initialised to opaque red. `text` is applied as the
/// control's label.
pub fn create_color_well(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ColorWell::new(Color::rgba(255, 0, 0, 255), geometry)))
}

#[cfg(full_widgets)]
/// Creates a tag entry control with no tags. `text` is applied as the control's
/// label.
pub fn create_tag_input(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TagInput::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a pre-edit display for input-method composition with no text. `text`
/// is applied as the control's label.
pub fn create_ime_preedit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ImePreedit::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an in-place editor over an empty cell, so it has no value to edit
/// until one is set. `text` is applied as the control's label.
pub fn create_inplace_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(InplaceEditor::new("", geometry)))
}

#[cfg(full_widgets)]
/// Creates a QR code view with no payload encoded. `text` is applied as the
/// control's label, not as the content to encode.
pub fn create_qr_code(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(QRCode::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a masonry layout with no child tiles. `text` is applied as the
/// control's label.
pub fn create_masonry_layout(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MasonryLayout::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a Material-styled snackbar in the dismissed state. `text` is applied
/// as the control's label.
pub fn create_material_snackbar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MaterialSnackbar::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an adaptive scaffold with an empty body. `text` is applied through
/// the label contract rather than being passed to the constructor.
pub fn create_adaptive_scaffold(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(AdaptiveScaffold::new("", geometry)))
}

#[cfg(full_widgets)]
/// Creates a wizard dialog at its first page with no pages defined. `text` is
/// applied as the control's label.
pub fn create_wizard_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(WizardDialog::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a safe area inset with no padding applied. `text` is applied as the
/// control's label.
pub fn create_safe_area(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SafeArea::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an iOS-style alert dialog with no title, message or actions. `text`
/// is applied as the control's label.
pub fn create_cupertino_alert_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CupertinoAlertDialog::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an iOS-style slider with the control's default range. `text` is
/// applied as the control's label.
pub fn create_cupertino_slider(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CupertinoSlider::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a tooltip with an empty tip text. `text` is applied through the label
/// contract rather than being passed to the constructor.
pub fn create_tooltip(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Tooltip::new("", geometry)))
}

#[cfg(full_widgets)]
/// Creates a segmented button group with no segments. `text` is applied as the
/// control's label.
pub fn create_segmented_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SegmentedButton::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a navigation stack with no pushed routes. `text` is applied as the
/// control's label.
pub fn create_navigation_stack(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(NavigationStack::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a circular progress indicator at zero. `text` is applied as the
/// control's label.
pub fn create_progress_circle(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ProgressCircle::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an icon view with no icon selected. `text` is applied as the
/// control's label.
pub fn create_icon(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Icon::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a drop-down menu with no items. `text` is applied as the control's
/// label.
pub fn create_dropdown_menu(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DropdownMenu::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a masked text entry with the control's default (unmasked) pattern.
/// `text` is applied as the control's label.
pub fn create_masked_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MaskedEdit::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a button that opens an attached menu, with an empty caption. `text`
/// is applied through the label contract rather than being passed to the
/// constructor.
pub fn create_menu_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MenuButton::new("", geometry)))
}

#[cfg(full_widgets)]
/// Creates a popover with no content, initially hidden. `text` is applied as the
/// control's label.
pub fn create_popover(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Popover::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a text entry offering completion suggestions, with no suggestions
/// loaded. `text` is applied as the control's label.
pub fn create_auto_complete_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(AutoCompleteEdit::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a combo box that allows more than one selection, with no items.
/// `text` is applied as the control's label.
pub fn create_multi_select_combo_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MultiSelectComboBox::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a slider with two handles spanning the control's default range.
/// `text` is applied as the control's label.
pub fn create_range_slider(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(RangeSlider::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a label that floats above its field once the field has content.
/// `text` is applied as the control's label.
pub fn create_floating_label(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FloatingLabel::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a preview of a font, defaulting to Arial. `text` is applied as the
/// control's label, not as the sample string the preview renders.
pub fn create_font_preview(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FontPreview::new("Arial", geometry)))
}

#[cfg(full_widgets)]
/// Creates an iOS-style navigation bar with an empty title. `text` is applied as
/// the control's label.
pub fn create_cupertino_navigation_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CupertinoNavigationBar::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an iOS-style segmented control with no segments. `text` is applied as
/// the control's label.
pub fn create_cupertino_segmented_control(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CupertinoSegmentedControl::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a pull-to-refresh control in its idle state. `text` is applied as the
/// control's label.
pub fn create_refresh_control(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(RefreshControl::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a modal bottom sheet in its collapsed state. `text` is applied as the
/// control's label.
pub fn create_modal_bottom_sheet(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ModalBottomSheet::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a find-and-replace dialog with empty search and replacement fields.
/// `text` is applied as the control's label.
pub fn create_find_replace_dialog(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(FindReplaceDialog::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a properties panel with no property rows. `text` is applied as the
/// control's label.
pub fn create_properties_panel(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(PropertiesPanel::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an iOS-style date picker opened on the current date. `text` is
/// applied as the control's label.
pub fn create_cupertino_date_picker(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CupertinoDatePicker::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a combo box whose text may also be typed, with no items. `text` is
/// applied as the control's label.
pub fn create_editable_combo_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(EditableComboBox::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a picker for a start and end date, with no range selected. `text` is
/// applied as the control's label.
pub fn create_date_range_picker(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(DateRangePicker::new(geometry)))
}

// ── New widget constructors (non-mini) ───────────────────────────

#[cfg(full_widgets)]
/// Creates a rich text editor with no document loaded. `text` is applied as the
/// control's label.
pub fn create_rich_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(RichEdit::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a carousel at its first item with no items defined. `text` is applied
/// as the control's label.
pub fn create_carousel(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Carousel::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a Material navigation rail with no destinations. `text` is applied as
/// the control's label.
pub fn create_material_navigation_rail(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(MaterialNavigationRail::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a tab view with no tabs. `text` is applied as the control's label.
pub fn create_tab_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(TabView::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a search bar with an empty query. `text` is applied as the control's
/// label.
pub fn create_search_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SearchBar::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a keyboard-shortcut capture field with no shortcut assigned. `text`
/// is applied as the control's label.
pub fn create_shortcut_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ShortcutEditor::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a swipe-to-dismiss wrapper with no content. `text` is applied as the
/// control's label.
pub fn create_swipe_to_dismiss(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(SwipeToDismiss::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a line chart with no series. `text` is applied as the control's
/// label.
pub fn create_line_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(LineChart::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a sparkline with no data points. `text` is applied as the control's
/// label.
pub fn create_sparkline(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(Sparkline::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a bar chart with no series. `text` is applied as the control's label.
pub fn create_bar_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(BarChart::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a pie chart with no slices. `text` is applied as the control's label.
pub fn create_pie_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(PieChart::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a view for a multi-frame image with no frames loaded. `text` is
/// applied as the control's label.
pub fn create_animated_image(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(AnimatedImage::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates the host for a shared-element (hero) transition with no elements
/// registered. `text` is applied as the control's label.
pub fn create_hero_animation(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(HeroAnimation::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an editor for a cubic Bézier easing curve, holding no control
/// points yet. `text` is applied as the control's label.
pub fn create_bezier_curve_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(BezierCurveEditor::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a view for a Lottie animation with no animation loaded. `text` is
/// applied as the control's label.
pub fn create_lottie_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(LottieWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a view for a Rive animation with no animation loaded. `text` is
/// applied as the control's label.
pub fn create_rive_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(RiveWidget::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a video player with no media loaded and playback stopped. `text` is
/// applied as the control's label.
pub fn create_video_player(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(VideoPlayer::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an image gallery with no images. `text` is applied as the control's
/// label.
pub fn create_image_gallery(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(ImageGallery::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an audio visualiser with no audio source attached. `text` is applied
/// as the control's label.
pub fn create_audio_visualizer(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(AudioVisualizer::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a live camera preview with no camera opened. `text` is applied as the
/// control's label.
pub fn create_camera_preview(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(CameraPreview::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a barcode scanner view with no scan in progress. `text` is applied as
/// the control's label.
pub fn create_barcode_scanner(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(BarcodeScanner::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a tool bar button labelled `text`.
pub fn create_tool_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(ToolButton::new(text.to_string(), geometry))
}

#[cfg(full_widgets)]
/// Creates a status bar with no message or indicators. `text` is applied as the
/// control's label.
pub fn create_status_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(StatusBar::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates a property grid with no property rows. `text` is applied as the
/// control's label.
pub fn create_property_grid(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(geometry, text, Box::new(PropertyGrid::new(geometry)))
}

#[cfg(full_widgets)]
/// Creates an empty [`CandlestickChart`](crate::widget::special_widgets::finance::candlestick_chart::CandlestickChart).
/// `text` is applied as a label beside the control, following the convention the
/// other chart constructors use.
#[allow(unused_mut)]
pub fn create_candlestick_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let widget =
        crate::widget::special_widgets::finance::candlestick_chart::CandlestickChart::new(geometry);
    label(geometry, text, Box::new(widget))
}

#[cfg(full_widgets)]
/// Creates an empty [`VolumeChart`](crate::widget::special_widgets::finance::volume_chart::VolumeChart).
/// `text` is applied as a label beside the control, following the convention the
/// other chart constructors use.
#[allow(unused_mut)]
pub fn create_volume_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let widget = crate::widget::special_widgets::finance::volume_chart::VolumeChart::new(geometry);
    label(geometry, text, Box::new(widget))
}

#[cfg(full_widgets)]
/// Creates an empty [`DepthChart`](crate::widget::special_widgets::finance::depth_chart::DepthChart).
/// `text` is applied as a label beside the control, following the convention the
/// other chart constructors use.
#[allow(unused_mut)]
pub fn create_depth_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let widget = crate::widget::special_widgets::finance::depth_chart::DepthChart::new(geometry);
    label(geometry, text, Box::new(widget))
}

#[cfg(full_widgets)]
/// Creates an empty `OrderBookWidget`.
/// `text` is applied as a label beside the control, following the convention the
/// other chart constructors use.
#[allow(unused_mut)]
pub fn create_order_book(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let widget =
        crate::widget::special_widgets::finance::order_book::OrderBookWidget::new(geometry);
    label(geometry, text, Box::new(widget))
}

#[cfg(full_widgets)]
/// Creates an empty [`QuoteBoard`](crate::widget::special_widgets::finance::quote_board::QuoteBoard).
/// `text` is applied as a label beside the control, following the convention the
/// other chart constructors use.
#[allow(unused_mut)]
pub fn create_quote_board(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let widget = crate::widget::special_widgets::finance::quote_board::QuoteBoard::new(geometry);
    label(geometry, text, Box::new(widget))
}

#[cfg(full_widgets)]
/// Creates an empty [`IndicatorChart`](crate::widget::special_widgets::finance::indicator_chart::IndicatorChart).
/// `text` is applied as a label beside the control, following the convention the
/// other chart constructors use.
#[allow(unused_mut)]
pub fn create_indicator_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let widget =
        crate::widget::special_widgets::finance::indicator_chart::IndicatorChart::new(geometry);
    label(geometry, text, Box::new(widget))
}
